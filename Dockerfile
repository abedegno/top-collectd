# Stage 1: Build the plugin
FROM rust:1.83-bookworm AS builder

WORKDIR /usr/src/top-collectd
COPY Cargo.toml ./
COPY src ./src

RUN cargo build --release

# Stage 2: Test with collectd
FROM ubuntu:24.04 AS test

RUN apt-get update && \
    apt-get install -y --no-install-recommends collectd-core && \
    rm -rf /var/lib/apt/lists/*

# Copy the built plugin
COPY --from=builder /usr/src/top-collectd/target/release/libtop.so /usr/lib/collectd/top.so

# Copy test configuration and script
COPY test/collectd.conf /etc/collectd/collectd.conf
COPY test/run-test.sh /usr/local/bin/run-test.sh
RUN chmod +x /usr/local/bin/run-test.sh

ENTRYPOINT ["/usr/local/bin/run-test.sh"]

# Stage 3: Collectd with RRD support (for CGP testing)
FROM test AS collectd-rrd

RUN apt-get update && \
    apt-get install -y --no-install-recommends rrdtool librrd8t64 && \
    rm -rf /var/lib/apt/lists/*

ENTRYPOINT ["collectd", "-f"]

# Stage 4: CGP frontend for interactive testing
FROM ubuntu:24.04 AS cgp

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        apache2 \
        libapache2-mod-php \
        php-json \
        rrdtool \
        git \
        ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Clone CGP into the web root
RUN git clone --depth 1 https://github.com/pommi/CGP.git /var/www/html/cgp

# Configure CGP to read RRD data from the shared volume
RUN printf "<?php\n\$CONFIG['datadir'] = '/var/lib/collectd/rrd';\n\$CONFIG['time_range']['default'] = 3600;\n" \
    > /var/www/html/cgp/conf/config.local.php

# Install our custom top plugin template
COPY cgp/plugin/top.json /var/www/html/cgp/plugin/local/top.json

# Apache runs in foreground
ENV APACHE_RUN_USER=www-data
ENV APACHE_RUN_GROUP=www-data

EXPOSE 80
CMD ["apachectl", "-D", "FOREGROUND"]
