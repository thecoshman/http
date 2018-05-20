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

    Only matters if --allow-write is also specified or --no-encode is not.

    Default: $TEMP.

  --ssl [TLS_IDENTITY_FILE]

    TLS identity file to use to encrypt as.

    The password is taken from the HTTP_SSL_PASS environment variable, or empty
    if that variable doesn't exist.

    Default: None.

  --gen-ssl

    Generate a passwordless, single-use TLS self-signed certificate
    and use it for this session.

    Exclusive with --ssl. Default: false.

  -s --no-follow-symlinks

    Don't follow symlinks when requesting file access.

    If a symlink is requested and this flag is on it will be treated as if it
    didn't exist.

  -r --sandbox-symlinks

    Restrict/sandbox where symlinks lead to only the direct descendants
    of the hosted directory.

    If a file outside the direct descendancy of the hosted reictory requested
    and this flag is on it will be treated as if it didn't exist.

  -w --allow-write

    Allow for write operations.

    Currently supported write operations: PUT and DELETE.

    This is false by default because it's most likely not something you
    want to do.

  -i --no-indices

    Always generate directory listings, even for directories containing an
    index file.

    This is false by default because it's most likely for debugging purposes.

  -e --no-encode

    Do not encode filesystem files.

    Encoded files are stored in the temp directory rather than being kept in
    memory.

    This is false by default because it's useful for reducing bandwidth usage.

## EXAMPLES

  `http`

    Host the current directory on the first free port upwards of 8000,
    don't follow symlinks, don't allow writes.

    Example output:
      p:\Rust\http> http
      Hosting "." on port 8000 without TLS...
      Ctrl-C to stop.

      127.0.0.1:47880 was served directory listing for \\?\P:\Rust\http
      127.0.0.1:47902 was served file \\?\P:\Rust\http\http.1.html as text/html
      127.0.0.1:47916 was served file S:\Rust-target\doc\main.css as text/css
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

    As in the first example, but don't follow symlinks.

    Example output change:
      127.0.0.1:47916 requested to GET nonexistant entity S:\Rust-target\doc\main.css

  `http -w`

    As in the first example, but allow writes.

    Example output change:
      127.0.0.1:47936 deleted file \\?\P:\Rust\http\http.1.html
      127.0.0.1:48222 created \\?\P:\Rust\http\index.html, size: 1033554B

    Corresponding request:
      DELETE /http.1.html
      PUT /index.html with request body containing roughly 1MB of data

    Another behavioral change is that, in this case, the folder (and file)
    named "T:/-=- TEMP -=-/http-P-Rust-http/writes/" and
    "T:/-=- TEMP -=-/http-P-Rust-http/writes/index.html" were created while the file
    "P:\Rust\http\http.1.html" was deleted (also works on directories).

  `http -w -t "../TEMP"`

    As in the previous example, but use a different temp dir.

    Behavioral changes: the created folder and file are
    named "P:/Rust/TEMP/http-P-Rust-http/writes/" and
    "P:/Rust/TEMP/http-P-Rust-http/writes/index.html".

  `http -p 6969`

    As in the first example, but host on port 6969.

    Assuming the port is free, example output change:
      Hosting "." on port 6969 without TLS...

    If the port is taken, example output change:
      Starting server failed: port taken.
      <EOF>

  `HTTP_SSL_PASS=pwd http --ssl cert/http8k.p12`

    As in the first example, but encrypt with the identity file cert/http8k.p12
    unlocked with password "pwd".

    Assuming password is correct, example output change:
      Hosting "." on port 8000 TLS certificate from "cert/http8k.p12"...

  `http --gen-ssl`

    As in the first example, but encrypt with a newly created self-signed
    identity file.

    Example output change:
      Hosting "." on port 8000 TLS certificate from "$TEMP/http-P-Rust-http/tls/tls.p12"...

  `http -r`

    As in the first example, but restrict accessible paths
    to direct descendants of the hosted directory.

    Example output change:
      127.0.0.1:47916 requested to GET nonexistant entity S:\Rust-target\doc\main.css

## AUTHOR

Written by thecoshman &lt;<thecoshman@gmail.com>&gt;,
       and nabijaczleweli &lt;<nabijaczleweli@gmail.com>&gt;.

## REPORTING BUGS

&lt;<https://github.com/thecoshman/http/issues>&gt;

## SEE ALSO

&lt;<https://github.com/thecoshman/http>&gt;
