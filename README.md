# top-collectd

[![CI](https://github.com/abedegno/top-collectd/actions/workflows/ci.yml/badge.svg)](https://github.com/abedegno/top-collectd/actions/workflows/ci.yml)

A native [collectd](https://collectd.org/) reader plugin written in Rust that reports the top processes by CPU usage. Each read interval, the plugin samples process CPU times over a 500ms window and submits the top 10 processes as gauge values representing percentage CPU usage.

## How It Works

1. Reads all process CPU times from `/proc/[pid]/stat`
2. Waits 500ms and reads them again
3. Calculates per-process CPU usage as a percentage (scaled by number of CPUs)
4. Submits the top 10 processes to collectd

## Compatibility

This plugin reads from `/proc`, so it only works on **Linux** systems.

Tested with:
- collectd 5.7+

## Building

Ensure you have [Rust installed](https://rustup.rs/), then:

```sh
cargo build --release
```

The compiled shared library will be at `./target/release/libtop.so`.

> **Note:** As of collectd-plugin 0.14+, the `COLLECTD_VERSION` environment variable is no longer required — it defaults to the 5.7 API.

### Building with Docker

You can build without installing Rust locally. This compiles the plugin inside a container and copies `libtop.so` to `./dist/`:

```sh
docker compose --profile build up --build
```

### Testing with Docker

Run an integration test that starts collectd with the plugin and verifies it produces output:

```sh
docker compose --profile test up --build
```

### Build and test together

```sh
docker compose --profile build --profile test up --build
```

## Installation

1. Copy the shared library to your collectd plugin directory:

   ```sh
   sudo cp ./target/release/libtop.so /usr/lib64/collectd/top.so
   ```

2. Add the following to your `collectd.conf`:

   ```
   LoadPlugin top
   ```

   No additional plugin configuration is needed — the plugin has no configurable parameters.

3. Restart collectd:

   ```sh
   sudo systemctl restart collectd
   ```

## Collected Metrics

The plugin submits values with the following identifiers:

| Field             | Value                                    |
| ----------------- | ---------------------------------------- |
| Plugin            | `top`                                    |
| Type              | `percent`                                |
| Plugin Instance   | Process name (e.g. `httpd`, `postgres`)  |
| Type Instance     | `cpu`                                    |

Data is stored in RRD files at paths like:

```
<hostname>/top-<process>/percent-cpu.rrd
```

For example: `myserver/top-httpd/percent-cpu.rrd`

### Collectd Graph Panel (CGP)

A custom CGP plugin template is included in `cgp/plugin/top.json`. To install it, copy the file into CGP's local plugin override directory:

```sh
cp cgp/plugin/top.json /path/to/cgp/plugin/local/top.json
```

To test CGP locally with Docker, run:

```sh
docker compose --profile cgp up --build
```

Then open [http://localhost:8080/cgp/](http://localhost:8080/cgp/) in your browser. After ~20 seconds (one collectd read cycle), the `top` plugin will appear in the sidebar with per-process CPU graphs.

## License

MIT
