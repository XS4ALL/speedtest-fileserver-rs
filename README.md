# speedtest-xxx-mb

This is a server that serves random data. It accepts URLs of the form
`http://domain.name/<size><multiplier>.<ext>`.

For example, when running on localhost port 3000:

```
http://localhost:3000/10MB.bin
http://localhost:3000/2GiB.bin
http://localhost:3000/97KiB.bin
```

.. and it will serve a file of therequested size consisting of random data.

The directory index `http://domain.name/` serves a dirlisting of a
number of files with common sizes in the range of 1MB to 10GB.

## building it.

First install rust, then build it with `cargo build --release`. The
binary will be present in `target/release/speedtest_xxx_mb`.


