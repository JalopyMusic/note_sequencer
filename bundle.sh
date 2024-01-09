#!/bin/sh

set -eux

cargo xtask bundle note_sequencer --release
