#!/usr/bin/env bash

ls src/*.rs .cargo/* src/allocator/*.rs Cargo.toml | entr sh -c "cargo bootimage"
