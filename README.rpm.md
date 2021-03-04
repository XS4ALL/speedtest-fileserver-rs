## Building a `.rpm` file.

- First, we need to install a RPM and C development environment. So run:

```
sudo yum groupinstall 'Development Tools'
```

- Then, you need to be able to compile the server, so follow the instructions
  in the main README.md to install Rust, and clone the git repository.

- Now, install the `cargo rpm` subcommand:

```
cargo install cargo-rpm
````

- Then make sure you are `cd`'ed into the `speedtest-fileserver-rs` directory, and run:

```
cargo rpm build -v
```

There should be a RPM file in `target/release/rpmbuild/RPMs/x86_64`.

## Installing the `.rpm`:

- rpm -i `speedtest-fileserver-<version>-<os>-<arch>.rpm`
- edit /etc/speedtest-fileserver.cfg
- systemctl enable speedtest-fileserver
- systemctl start speedtest-fileserver

