#!/usr/bin/env bash

ls src/*.rs .cargo/* Cargo.toml | entr sh -c "cargo bootimage"
