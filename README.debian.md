## Building a Debian package.

This was developed on Debian/10 (buster).

First follow the instructions in the main README.md.

Then install the build tools we need:

```
sudo apt add build-essential debhelper dh-systemd fakeroot
```

Now run:

```
dpkg-buildpackage -rfakeroot -us -uc`
````

and there should be a `.deb` package in the parent directory.

After installing, you might want to customize `/etc/speedtest-fileserver.cfg`
and perhaps `/etc/logrotate.d/speedtest-fileserver.conf`.

