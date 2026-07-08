#compdef mantis

autoload -U is-at-least

_mantis() {
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext="$curcontext" state line
    _arguments "${_arguments_options[@]}" : \
'-l+[Force the syntax highlighting language for piped stdin]:LANG:_default' \
'--language=[Force the syntax highlighting language for piped stdin]:LANG:_default' \
'--completions=[Generate shell completions (bash, zsh, fish, powershell)]:SHELL:_default' \
'--print-man-page[Print the man page to stdout]' \
'--update[Self-update to the latest release]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'-V[Print version]' \
'--version[Print version]' \
'::path -- File or directory to open (default\: current directory):_files' \
&& ret=0
}

(( $+functions[_mantis_commands] )) ||
_mantis_commands() {
    local commands; commands=()
    _describe -t commands 'mantis commands' commands "$@"
}

if [ "$funcstack[1]" = "_mantis" ]; then
    _mantis "$@"
else
    compdef _mantis mantis
fi
