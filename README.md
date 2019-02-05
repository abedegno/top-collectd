# top-collectd

top-collectd provides a native collectd reader plugin to report top processes and their percentage CPU Usage. 

## Compatibility

This repo is tested on the following (though compatibility isn't limited to):
- collectd 5.8.1 (CentOS 7.6)

## Building

To build the repo for collectd, ensure you have [Rust installed](https://rustup.rs/)

```
COLLECTD_VERSION=5.7 cargo build --release
```

The resulting `./target/release/libtop.so` should be copied (locally or remotely) to `/usr/lib64/collectd/top.so`
