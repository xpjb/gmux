#!/bin/bash
# clean build
cargo build

# Kill existing processes to ensure a clean slate
pkill Xephyr
pkill gmux

# Start Xephyr in the background
Xephyr :1 -screen 1920x1080 &
XEPHYR_PID=$!

# Give Xephyr a moment to start up
sleep 1

# Set the display and run gmux
export DISPLAY=:1
./target/debug/gmux

# Clean up on exit
kill $XEPHYR_PID
