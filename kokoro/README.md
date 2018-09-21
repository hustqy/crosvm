# Kokoro CI for crosvm

For presubmit testing, each change posted for Gerrit on the master branch of crosvm will be tried by
Kokoro. The configuration is found in [`presubmit.cfg`](presubmit.cfg) and the build script is at
[`build.sh`](build.sh). A Docker image called `crosvm-base` is used as the testing environment which
is built with a [`Dockerfile`](Dockerfile).

[TOC]

## How to use Docker to test crosvm

Assuming a Docker daemon is already running, build the `crosvm-base` image:

```shell
cd crosvm/kokoro
docker build -t crosvm-base - < Dockerfile
```

Here is how to use the image to test a crosvm repository located at `$CROSVM_SRC`:

```shell
docker run --privileged -v /dev/log:/dev/log -v "${CROSVM_SRC}":/src:ro crosvm-base
```

> **WARNING**:
> The `--privileged` is so that the container will have `/dev/kvm` access.

## How to update `crosvm-base`

The `crosvm-base` `Dockerfile` downloads, builds, and install specific library versions needed to
test crosvm. It also defines a run time environment and default command line for performing a test.
If an update or new library is needed or any other adjustment is required, a new image can be
generated as follows:

```shell
cd crosvm/kokoro
docker build -t crosvm-base - < Dockerfile
docker save crosvm-base | xz -T 0 -z >crosvm-base.tar.xz
```

If you have x20 access, move `crosvm-base.tar.xz` to `/teams/chromeos-vm/docker/` and ensure the
owner is `chromeos-vm-ci-read-write`. This owner is used to allow Kokoro to read the base image in
during the test run. The updated image will be used for future Kokoro runs until it is replaced.

> **WARNING**:
> If the image tarball uploaded to x20 is defective in any way, Kokoro will fail to verify every
> crosvm change as if the change itself were defective. Please verify the image is good before
> uploading to x20.

## How to simulate Kokoro before uploading

If you want to test a change before uploading it in a similar environment to Kokoro, use the
[`kokoro_simulator.sh`](kokoro_simulator.sh) script. It will invoke the `build.sh` script after
exporting environment variables and a volume that are expected to be present. The crosvm source code
is symlinked in, and is tested exactly as in the working directory. Any changes to `build.sh` will
also be tested, but any changes to `presubmit.cfg` will have no effect. If there are any changes to
`Dockerfile`, they will have no effect unless the `crosvm-base` image is removed (or never existed)
from the local Docker daemon. To test `Dockerfile` changes use the following formula to purge
`crosvm-base`.

```shell
# So that kokoro_simulator.sh doesn't skip `docker save`.
rm /tmp/kokoro_simulator/crosvm-base.tar.xz

# Stopped containers prevent the removal of images below.
docker container prune

# So that kokoro_simulator.sh doesn't skip `docker build`.
docker rmi crosvm-base
```

心