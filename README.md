# speedtest-fileserver-rs

This is a server that serves random data. It accepts URLs of the form
`http://domain.name/<size><multiplier>.<ext>` (or https).

For example, when running on localhost port 4000:

```
http://localhost:4000/10MB.bin
http://localhost:4000/2GiB.bin
http://localhost:4000/97KiB.bin
```

.. and it will serve a file of the requested size consisting of random data.

The directory index `http://domain.name/` serves a dirlisting of a
number of files with common sizes in the range of 1MB to 10GB.

## building it.

First install rust:
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

then build it with
```
cargo build --release`
```

The binary will be present in `target/release/speedtest-fileserver-rs`.
There is an example configuration file provided, you need to
install that in `/etc` or tell the server where it lives with the
`--config` command line option.

## building a Debian package.

First install rust, then install the build tools we need:

```
apt add build-essential debhelper dh-systemd fakeroot
```

Now run `dpkg-buildpackage -r fakeroot -us uc` and there should
be a `.deb` package in the parent directory.

## configuration.

The server can be configured to serve eiter http or http, or both.
See the comments in the [example configuration file](speedtest-fileserver.cfg).

