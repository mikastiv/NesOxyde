# NesOxyde

A NES emulator 100% written in Rust

## Overview

This project is my first big programming project and also my first emulator (I'm not counting my Chip8 emulator because it was very simple compared to this one).

I chose Rust because it's fast and it's by far my favorite language. Also all the code is 100% safe Rust!

This was my 4th attempt at coding a NES emulator and I finally succeded! The emulator is not cycle accurate, but all the games I've tried work pretty well.

## Usage

The program is needs libsdl2 to run.
It works on Linux, Windows and MacOS

Launch: ./nesoxyde [SyncMode] \<iNES File\>

SyncMode:

- -A (default) - Audio sync. The emulation is synced with the audio sample rate (44100Hz). Can cause frame lag.
- -V - Video sync. The emulation is synced with the video refresh rate of 60fps. Can cause audio pops and cracks.

## Screenshots

![Super Mario Bros](/screenshots/smb.png "Super Mario Bros")
![Zelda](/screenshots/zelda.png "Zelda")
![Castlevania](/screenshots/castlevania.png "Castlevania")
