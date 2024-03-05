# Music Sync

A highly configurable audio oriented real time music synchronization program written in Rust.

## Getting started

Clone the repo and compile from source.

You need to have at least [Rust](https://www.rust-lang.org/) 1.75 installed.

You also need to have the following installed:

### Windows and Mac

You are good to go

### Linux

`openssl` needs to be installed. Instructions can be found [here](https://docs.rs/openssl/latest/openssl/).

On Ubuntu the following packages are also needed: \
`pkg-config` `libfreetype6-dev` `libfontconfig1-dev`.

Also the ALSA development files are required. These are provided as part of the `libasound2-dev` package on Debian and Ubuntu distributions and `alsa-lib-devel` on Fedora.

## Running the Program

If you are compiling from source code you can simply run `cargo run --release` in a command line and the project will be built and run. If you have a precompiled binary, you can just execute it on the command line as well.

### Configuration

You need to have a `config.toml` file in the same folder as the current shell location.

The file can be empty and you only need to write out the options you want to change.
An overview over all available options with their standard value can be found in the provided [config_template.toml](config_template.toml).

Currently syncing with Philips Hue Lamps and WLED Light strips is possible.

An example `config.toml` may look like:

```toml
[[Hue]]

[[WLED]]
effect = "Spectrum"
ip = "Ip of Strip"
```

With this config the program will automatically search for a Hue Bridge on the current network and initiate Push Link authentication and use the first Entertainment Area found for synchronization. It will also connect to the specified WLED strip show the spectrum effect.

## How is the audio processed?

Using the spectrogram of the audio an Onset detection function is calculated.
An Onset can be thought of as the start of a note, like a transient but more generalized.
An onset detection function tries to return higher values, if it thinks a that point might be an onset.
By picking the peaks of this function you get the final onsets.
Currently implemented are the High Frequency Content [[1]](#1) (HFC) algorithm and
a modified version of the spectral flux algorithm [[2]](#2).
Both have some rudimentary augmentations to allow to (poorly) differentiate between kick drum, snare drum and hihat.

## References

<a id="1">[1]</a>
Bello, Juan Pablo, et al.
"A tutorial on onset detection in music signals."
IEEE Transactions on speech and audio processing 13.5 (2005): 1035-1047.

<a id="2">[2]</a>
Böck, Sebastian, Florian Krebs, and Markus Schedl.
"Evaluating the Online Capabilities of Onset Detection Methods."
ISMIR. 2012.
