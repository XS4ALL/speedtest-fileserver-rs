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

First install rust:
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

You do need a recent rust version (from end 2020 or later).

then build the server with
```
cargo build --release`
```

The binary will be present in `target/release/speedtest-fileserver-rs`.
There is an example configuration file provided, you need to
install that in `/etc` or tell the server where it lives with the
`--config` command line option.

## Building a Debian package.

This was developed on Debian/10 (buster).

First install rust, then install the build tools we need:

```
sudo apt add build-essential debhelper dh-systemd fakeroot
```

Now run `dpkg-buildpackage -r fakeroot -us uc` and there should
be a `.deb` package in the parent directory.

After installing, you might want to customize `/etc/speedtest-fileserver.cfg`
and perhaps `/etc/logrotate.d/speedtest-fileserver`.

## Configuration.

The features mentioned above can be configured via the configuration file.
See the comments in the [example configuration file](speedtest-fileserver.cfg).

