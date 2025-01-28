#!/bin/sh

if [ $# -ne 2 ]; then
    echo "Usage: $0 <text> <test name>"
    exit 1
fi

argument_text="$1"

pipe_text=""
while IFS= read -r line; do
    pipe_text="${pipe_text}${line}\n"
done

if [ "$pipe_text" = "$argument_text" ]; then
    echo "PASS | $2"
    exit 0
else
    echo "FAIL | $2"
    echo "From pipe: $pipe_text"
    echo "From argument: $argument_text"
    exit 1
fi
