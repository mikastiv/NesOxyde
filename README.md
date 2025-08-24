# NesOxyde

A NES emulator 100% written in Rust

## Overview

This project is my first big coding project and also my first big emulator project.

I chose Rust because it's fast and it's by far my favorite language. Also all the code is 100% safe Rust!

The emulator is not cycle accurate, but all the games I've tried work pretty well. Based on the mappers I implemented, appart from a few exceptions, ~90% of games should work.

## Usage

The program needs libsdl2 to run and libsdl2-devel to compile.
It works on Linux, Windows and MacOS

Launch: ./nesoxyde [SyncMode] \<iNES File\>

SyncMode:

- Audio sync (default): The emulation is synced with the audio sample rate (44100Hz). Can cause frame lag.
- Video sync (-V): The emulation is synced with the video refresh rate of 60fps. Can cause audio pops and cracks.

## Controls

R -> Reset  
Esc -> Close emulator  
1 -> Volume down  
2 -> Volume up  
F1 -> Save state  
F2 -> Load state

Joypad:
- A -> B
- S -> A
- Z -> Select
- X -> Start
- UpArrow ->  Up
- DownArrow -> Down
- LeftArrow -> Left
- RightArrow -> Right

## Possible Improvements

- Make the CPU and PPU cycle accurate
- Change the sprite rendering routine to match what real hardware does
- Fix some mapper bugs (4 doesn't work with every game)

## Screenshots

![Super Mario Bros](/screenshots/smb.png "Super Mario Bros")
![Super Mario Bros 3](/screenshots/smb3.png "Super Mario Bros 3")
![Zelda](/screenshots/zelda.png "Zelda")
![Castlevania](/screenshots/castlevania.png "Castlevania")
