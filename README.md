# speedtest-fileserver-rs

This is a server that serves random data. It accepts URLs of the form
`http://domain.name/<size><multiplier>.<ext>` .

For example, when running on localhost port 3000:

```
http://localhost:3000/10MB.bin
http://localhost:3000/2GiB.bin
http://localhost:3000/97KiB.bin
```

.. and it will serve a file of the requested size consisting of random data.

The directory index `http://domain.name/` serves a dirlisting of a
number of files with common sizes in the range of 1MB to 10GB.

## Features.

- fast.
- serves completely random data.
- index file can be customized (handlebars template).
- http and https support.
- can write access log files.
- written in Rust without any unsafe code.

## Building it.

First install rust, if you haven't yet. Note that you need a recent version
of Rust. The version that comes with your OS might be too old.

Run this (as yourself or a development user, _not_ as root):
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then clone the repository, cd into it, and build the server:

then build the server with
```
git clone https://github.com/XS4ALL/speedtest-fileserver-rs
cd speedtest-fileserver-rs
cargo build --release
```

The binary will be present in `target/release/speedtest-fileserver-rs`.

There is an example configuration file provided, you need to
install that in `/etc` or tell the server where it lives with the
`--config` command line option.

## Building `.rpm` or `.deb` packages:

- [building a debian package](README.debian.md)
- [building a rpm](README.rpm.md)

## Configuration.

The features mentioned above can be configured via the configuration file.
See the comments in the [example configuration file](speedtest-fileserver.cfg).

## Bugs.

When access-log logging is enabled, the server logs download size and speed
(seconds elapsed, with ms accuracy). However due to internal buffering in the
libraries used and the OS itself, a logentry is written before all data has
been transferred to the client. The buffering might be a few MB, so using
size-of-download and time-elapsed to calculate download speeds is not
recommended, unless the download size is considerably larger than the
size of the internal buffers (say, 1GB and up).

If you need precision in the logs, try if a front-end proxy like nginx has
better accuracy.
