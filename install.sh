#!/bin/sh


if [ -z "${PREFIX+marker}" ]; then
  prefix_overriden=false;
else
  prefix_overriden=true;
fi

case "$(uname -s)" in
  CYGWIN*|MINGW32*|MSYS*)
    exe_suffix=.exe
    ;;

  *)
    exe_suffix=
    ;;
esac

PREFIX="${PREFIX:-"/usr/bin"}"
tag_name=$(curl -SsL "https://api.github.com/repos/thecoshman/http/releases/latest" | grep "tag_name" | head -1 | sed -e 's/.*": "//' -e 's/",//')


echo "Installing http $tag_name to $PREFIX..."
if [ "$prefix_overriden" = false ]; then
  echo "Set \$PREFIX environment variable to override installation directory.";
fi

mkdir -p "$PREFIX"
curl -SL "https://github.com/thecoshman/http/releases/download/$tag_name/http-$tag_name$exe_suffix" -o "$PREFIX/http$exe_suffix"

case "$(uname -s)" in
  CYGWIN*|MINGW32*|MSYS*)
    ;;

  *)
    chmod +x "$PREFIX/http$exe_suffix"
    ;;
esac
