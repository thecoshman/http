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

  -a --address [ADDRESS]

    IP to bind the server to.

    Default: 0.0.0.0.

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

  --auth [USERNAME[:PASSWORD]]

    Data for global authentication.

    Equivalent to --path-auth with a root path and the same crednetials.

    This argument is deprecated, and will be replaced with the current version
    of --path-auth on the next breaking release.
    Use --path-auth in new designs to avoid surprises.

    Default: None.

  --gen-auth

    Generate a one-off username:password set for global authentication.

    Functions as if --auth was specified with the generated credentials.

    This argument is deprecated, and will be replaced with the current version
    of --gen-path-auth on the next breaking release.
    Use --gen-path-auth in new designs to avoid surprises.

    Exclusive with --auth. Default: false.

  --path-auth [PATH=[USERNAME[:PASSWORD]]]

    Data for per-path authentication.

    The specified PATH will require the specified credentials to access.
    If credentials are unspecified, the path will have authentication
    disabled, even if global or parent paths have authentication specified.
    These can be arbitrarily nested.

    PATH is slash-normalised stripped of leading and trailing slashes.
    Specifying more than one of the same PATH is erroneous.

    Default: empty.

  --gen-path-auth [PATH]

    Generate a one-off username:password set for authentication under PATH.

    The username consists of 6-12 random alphanumeric characters, whereas
    the password consists of 10-25 random characters from most of the
    ASCII printable set.

    Functions as if --path-auth was specified with PATH
    and the generated credentials.

    Exclusive with --path-auth with the equivalent PATH. Default: empty.

  --proxy [HEADER-NAME:CIDR]

    Treat HEADER-NAME as a proxy forwarded-for header when the request
    originates from an address inside the network specified by the CIDR.

    If the header is set but the request isn't in the network, it's ignored.

    Can be specified any amount of times. Default: none.

  -m --mime-type [EXTENSION:MIME-TYPE]

    Return MIME-TYPE for files with EXTENSION.

    If EXTENSION is the empty string, return that MIME-TYPE for files with no extension.

    The default MIME type is as returned by the mime_guess crate, if any,
    otherwise "application/octet-stream" for binary files or "text/plain".

    Can be specified any amount of times. Default: none.

  --request-bandwidth [BYTES]

    Limit the band for each request to BYTES/second wide.

    Can be suffixed with [KMGTPE] binary prefixes or [kmgtpe] SI prefixes.
    Zero disables capping.

    Default: 0.

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

  -l --no-listings

    Do not generate directory listings.

    Behaviour table of --no-listings with --no-indices:
    +------------+---------+---------+---------+---------+
    |    Path    | Neither |   -i    |   -l    |  -l -i  |
    +============+=========+=========+=========+=========+
    | /has-index |  index  | listing |  index  |   404   |
    | /no-index  | listing | listing |   404   |   404   |
    +------------+---------+---------+---------+---------+

    This is false by default because it's most likely for debugging purposes.

  -i --no-indices

    Do not automatically serve the index file for directories containing one.

    This is false by default because it's most likely for debugging purposes.

  -e --no-encode

    Do not encode filesystem files.

    Encoded files are stored in the temp directory rather than being kept in
    memory.

    This is false by default because it's useful for reducing bandwidth usage.

  -x --strip-extensions

    Allow stripping index extentions from served paths:
    a request to /file might get served by /file.[s]htm[l].

    This is false by default

  -q --quiet...

    Suppress increasing amounts of output.

    Specifying this flag N times will, for:
      N == 0 – show all output
      N >= 1 – suppress serving status lines ("IP was served something")
      N >= 2 – suppress startup except for auth data, if present
      N >= 3 – suppress all startup messages

  -c --no-colour

    Don't colourise log output.

  -d --webdav

    Handle WebDAV requests.

    False by default.

## EXAMPLES

  `http`

    Host the current directory on the first free port upwards of 8000,
    don't follow symlinks, don't allow writes.

    Example output:
      p:\Rust\http> http
      Hosting "." on port 8000 without TLS and no authentication...
      Ctrl-C to stop.

      127.0.0.1:47880 was served directory listing for \\?\P:\Rust\http
      127.0.0.1:47902 was served file \\?\P:\Rust\http\http.1.html as text/html
      127.0.0.1:47916 was served file S:\Rust-target\doc\main.css as text/css
      127.0.0.1:48049 asked for OPTIONS
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
      127.0.0.1:47916 requested to GET nonexistent entity S:\Rust-target\doc\main.css

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
      Hosting "." on port 6969 without TLS and no authentication...

    If the port is taken, example output change:
      Starting server failed: port taken.
      <EOF>

  `http -a 192.168.65.1`

    As in the first example, but listen on address 192.168.65.1.

    Assuming the address can be used, example output change:
      Hosting "." on port 8000 under address 192.168.65.1 without TLS and no authentication...

    If the address is inaccessible or doesn't exist, example output change:
      Starting server failed: The requested address is not valid in its context.
      (os error 10049).
      <EOF>

  `HTTP_SSL_PASS=pwd http --ssl cert/http8k.p12`

    As in the first example, but encrypt with the identity file cert/http8k.p12
    unlocked with password "pwd".

    Assuming password is correct, example output change:
      Hosting "." on port 8000 TLS certificate from "cert/http8k.p12"
      and no authentication...

  `http --gen-ssl`

    As in the first example, but encrypt with a newly created self-signed
    identity file.

    Example output change:
      Hosting "." on port 8000 with TLS certificate from
      "$TEMP/http-P-Rust-http/tls/tls.p12" and no authentication...

  `http --path-auth /=Pirate`

    As in the first example, but require all clients to log in with the username "Pirate".

    Example output change:
      Hosting "." on port 8000 without TLS and basic authentication...
      Basic authentication credentials:
      Path  Username  Password
      /     Pirate

    On unauthed request:
      127.0.0.1:15141 requested to GET http://127.0.0.1:8005/ without authorisation

    Invalid credentials supplied:
      127.0.0.1:15142 requested to GET http://127.0.0.1:8005/ with invalid credentials
      "Pirate:memelord11"

    Valid credentials supplied:
      127.0.0.1:15142 correctly authorised to GET http://127.0.0.1:8005/
      127.0.0.1:15142 was served directory listing for \\?\P:\Rust\http

  `http --path-auth /=Pirate:memelord42`

    As in the first example, but require all clients to log in
    with the username "Pirate" and password "memelord42".

    Example output change:
      Hosting "." on port 8000 without TLS and basic authentication...
      Basic authentication credentials:
      Path  Username  Password
      /     Pirate    memelord42

    See above for log messages when performing requests.

  `http --gen-path-auth /`

    As in the first example, but generate a username:password pair
    and require all clients to log in therewith.

    Example output change:
      Hosting "." on port 8000 without TLS and basic authentication...
      Basic authentication credentials:
      Path  Username  Password
      /     jOvm8yCp  &gK=h&$-$HElLPb9HO%

    See above for log messages when performing requests.

  `http --path-auth /=admin:admin --gen-path-auth /target/debug --path-auth target/doc= --path-auth target/.rustc_info.json= --path-auth target/release/=releases`

    As in the first example, but allow unauthenticated access to /target/doc and /target/.rustc_info.json,
    require username "releases" and no password to access /target/release,
    require a randomly-generated username:password pair to access /target/debug,
    and lock all other paths behind "admin:admin".

    Example output change:
      Hosting "." on port 8000 without TLS and basic authentication...
      Basic authentication credentials:
      Path                      Username  Password
      /                         admin     admin
      /target/.rustc_info.json
      /target/debug             PYld448   l=Z~vdp,zAt^<uvRyU.T<F
      /target/doc
      /target/release           releases

    See above for log messages when performing requests.

  `http -r`

    As in the first example, but restrict accessible paths
    to direct descendants of the hosted directory.

    Example output change:
      127.0.0.1:47916 requested to GET nonexistent entity S:\Rust-target\doc\main.css

  `http --proxy X-Forwarded-For:127.0.0.1 --proxy X-Proxied-For:192.168.1.0/24`

    As in the first example, but treat the X-Forwarded-For and X-Proxied-For
    as proxy headers for requests from localhost and the 192.168.1.0/24 network,
    respectively.

    Given, that requests from 127.0.0.1, 192.168.1.109, and 93.184.216.34
    have the following headers set:
      X-Forwarded-For: OwO
      X-Proxied-For: UwU

    Then the output will be as follows:
      Hosting "." on port 8000 without TLS and no authentication...
      Trusted proxies:
      Header           Network
      X-Forwarded-For  127.0.0.1
      X-Proxied-For    192.168.1.0/24
      Ctrl-C to stop.

      [2020-02-17 17:48:41] 127.0.0.1:1392 for OwO was served directory listing for \\?\P:\Rust\http
      [2020-02-17 17:49:12] 192.168.1.109:1403 for UwU was served directory listing for \\?\P:\Rust\http
      [2020-02-17 17:49:29] 93.184.216.34:1397 was served directory listing for \\?\P:\Rust\http

  `http --mime-type css:text/css;charset=utf-8 --mime-type :image/jpeg`

    As in the first example, but send .css with the charset=utf8 attribute and
    treat files with no extension as JPEGs.

    Then the output will be as follows:
      Hosting "." on port 8000 without TLS and no authentication...
      Serving files with no extension as image/jpeg.
      Serving files with .css extension as text/css; charset=utf-8.
      Ctrl-C to stop.

      [2020-07-20 12:32:24] 127.0.0.1:47916 was served file \\?\S:\Rust-target\doc\main.css as text/css; charset=utf-8
      [2020-07-20 12:32:25] 127.0.0.1:2803 was served file \\?\P:\121D800E\http\DSC_6505 as image/jpeg

  `http --request-bandwidth 4K`

    As in the first example, but limit each request to 4096 bytes per second.

    Example output change:
      Hosting "." on port 8000 without TLS and no authentication...
      Requests limited to 4096B/s.
      Ctrl-C to stop.

## AUTHOR

Written by thecoshman &lt;<rust@thecoshman.com>&gt;,
           nabijaczleweli &lt;<nabijaczleweli@nabijaczleweli.xyz>&gt;,
           pheki,
           Adrian Herath &lt;<adrianisuru@gmail.com>&gt;,
       and cyqsimon.

## REPORTING BUGS

&lt;<https://github.com/thecoshman/http/issues>&gt;

## SEE ALSO

&lt;<https://github.com/thecoshman/http>&gt;
