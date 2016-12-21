http(1) -- a basic HTTP server for hosting a folder fast and simply
===================================================================

## SYNOPSIS

`http` [OPTIONS] [DIRECTORY]

## DESCRIPTION

Host These Things Please - a basic HTTP server for hosting a folder fast and
simply.

The idea is to make a program that can compile down to a simple binary that can
be used via Linux CLI to quickly take the current directory and serve it over
HTTP. Everything should have sensible defaults such that you do not *have* to
pass parameters like what port to use.

## OPTIONS

  [DIR]

    Directory to host. Must exist.

    Default: current working directory.

  -p --port [PORT]

    Port to host the server on.

    Must be between 1 and 65'535. Value of 0 will assign a random port,
    chosen by the OS.

    Default: first free port from 8000 up.

  -t --temp-dir [TEMP]

    Temporary directory to use to store data to write.

    Only matters if --allow-write is also specified.

    Default: $TEMP.

  -s --follow-symlinks

    Follow symlinks when requesting file access.

    This is false by default because it is most likely a problem (according to
    thecoshman anyway).

    If a symlink is requested and this is not on it will be treated as if it
    didn't exist.

  -w --allow-write

    Allow for write operations.

    Currently supported write operations: PUT and DELETE.

    This is false by default because it's most likely not something you
    want to do.

## EXAMPLES

  `http`

    Host the current directory on the first free port upwards of 8000,
    don't follow symlinks, don't allow writes.

    Example output:
      p:\Rust\http> http
      Hosting "." on port 8000...
      Ctrl-C to stop.

      127.0.0.1:47880 was served directory listing for \\?\P:\Rust\http
      127.0.0.1:47902 was served file \\?\P:\Rust\http\http.1.html as text/html
      127.0.0.1:47916 requested to GET nonexistant entity S:\Rust-target\doc\main.css
      127.0.0.1:48049 asked for options
      127.0.0.1:47936 used disabled request method DELETE
      127.0.0.1:48222 used disabled request method PUT
      ^C

    Assuming that `P:\Rust\http\target` is a symlink to `S:\Rust-target`,
    the following requests have been made, in order:

      GET /
      GET /http.1.html
      GET /target/doc/main.css
      OPTIONS
      DELETE <path doesn't matter>
      PUT <path doesn't matter>

    The above output snippet is used as a reference for other examples.

  `http -s`

    As in the first example, but follow symlinks.

    Example output change:
      127.0.0.1:47916 was served file S:\Rust-target\doc\main.css as text/css

  `http -w`

    As in the first example, but allow writes.

    Example output change:
      127.0.0.1:47936 deleted file \\?\P:\Rust\http\http.1.html
      127.0.0.1:48222 created \\?\P:\Rust\http\index.html, size: 1033554B

    Corresponding request:
      DELETE /http.1.html
      PUT /index.html with request body containing roughly 1MB of data

    Another behavioral change is that, in this case, the folder (and file)
    named "T:/-=- TEMP -=-/http-P-Rust-http/" and
    "T:/-=- TEMP -=-/http-P-Rust-http/index.html" were created while the file
    "P:\Rust\http\http.1.html" was deleted (also works on directories).

  `http -w -t "../TEMP"`

    As in the previous example, but use a different temp dir.

    Behavioral changes: the created folder and file are
    named "P:/Rust/TEMP/http-P-Rust-http/" and
    "P:/Rust/TEMP/http-P-Rust-http/index.html".

  `http -p 6969`

    As in the first example, but host on port 6969.

    Assuming the port is free, example output change:
      Hosting "." on port 6969...

    If the port is taken, example output change:
      Starting server failed: port taken.
      <EOF>

## AUTHOR

Written by thecoshman &lt;<thecoshman@gmail.com>&gt;,
       and nabijaczleweli &lt;<nabijaczleweli@gmail.com>&gt;.

## REPORTING BUGS

&lt;<https://github.com/thecoshman/http/issues>&gt;

## SEE ALSO

&lt;<https://github.com/thecoshman/http>&gt;
