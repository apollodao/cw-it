#!/bin/sh

# Escape codes
RESET="\x1b[0m"
RED="\x1b[31;49m"
GREEN="\x1b[32;49m"
BD="\x1b[39;49;1m"
IT="\x1b[39;49;3m"
UL="\x1b[39;49;4m"

# if path is supplied as argument
if [[ ! -z $1 && -d $1 ]]; then
    GREP_DIR=$1
    echo "${BD}Searching in '$RESET$UL$GREP_DIR$RESET$BD'...$RESET"
else
    GREP_DIR="."
    echo "${BD}No path supplied. Defaulting to current working directory...$RESET"
fi

# Regex
LINT="todo[^!]"
FORMAT="s/\.\/([a-zA-Z0-9_/.-]+):([0-9]+):(.+)/$UL\1$RESET ${BD}@ line \2:$RESET\n\t$IT$RED\3$RESET/"

N=$(grep -riIo --include=*.{rs,ts,js} -E $LINT $GREP_DIR | wc -l | xargs)


if [ $N -gt 0 ]; then
    echo "${BD}Found $UL$RED$N$RESET$BD occurrences matching pattern '$RESET$IT$LINT$RESET$BD':$RESET"
    echo "------------------------------------------------"
    grep -rniI --include=*.{rs,ts,js} -E $LINT $GREP_DIR | sed -E "$FORMAT"
    exit 1
fi

echo "${GREEN}No occurrences of pattern '$IT$LINT$RESET$GREEN' found!$RESET"
exit 0
