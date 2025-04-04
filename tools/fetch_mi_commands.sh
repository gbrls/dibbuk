#!/usr/bin/env bash

curl -s https://raw.githubusercontent.com/bminor/binutils-gdb/refs/heads/master/gdb/mi/mi-cmds.c | grep add_mi_cmd_mi | cut -d '"' -f2 | grep -v 'mi ('
