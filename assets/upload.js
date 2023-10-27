"use strict";

window.addEventListener("load", function() {
  const SUPPORTED_TYPES = ["Files", "application/x-moz-file"];

  let body = document.getElementsByTagName("body")[0];
  let file_upload = document.getElementById("file_upload");
  let file_upload_text = null;
  let remaining_files = 0;
  let url = document.location.pathname;
  if(!url.endsWith("/"))
    url += "/";

  body.addEventListener("dragover", function(ev) {
    if(SUPPORTED_TYPES.find(function(el) {
      return (ev.dataTransfer.types.contains || ev.dataTransfer.types.includes).call(ev.dataTransfer.types, el);
    }))
      ev.preventDefault();
  });

  body.addEventListener("drop", function(ev) {
    if(SUPPORTED_TYPES.find(function(el) {
      return (ev.dataTransfer.types.contains || ev.dataTransfer.types.includes).call(ev.dataTransfer.types, el);
    })) {
      ev.preventDefault();

      for(let i = ev.dataTransfer.files.length - 1; i >= 0; --i) {
        if(!ev.dataTransfer.items[i].webkitGetAsEntry) {
          let file = ev.dataTransfer.files[i];
          upload_file(url + encodeURIComponent(file.name), file);
        } else
          recurse_upload(ev.dataTransfer.items[i].webkitGetAsEntry(), url);
      }
    }
  });

  file_upload.addEventListener("change", function() {
    for(let i = file_upload.files.length - 1; i >= 0; --i) {
      let file = file_upload.files[i];
      upload_file(url + encodeURIComponent(file.name), file);
    }
  });

  function upload_file(req_url, file) {
    ++remaining_files;
    if(!file_upload_text) {
      file_upload_text = document.createTextNode(1);
      file_upload.parentNode.insertBefore(file_upload_text, file_upload.nextSibling); // insertafter
    } else
      file_upload_text.data = remaining_files;

    let request = new XMLHttpRequest();
    request.addEventListener("loadend", function(e) {
      if(!--remaining_files)
        window.location.reload();
      file_upload_text.data = remaining_files;
    });
    request.open("PUT", req_url);
    if(file.lastModified)
      request.setRequestHeader("X-Last-Modified", file.lastModified);
    request.send(file);
  }

  function recurse_upload(entry, base_url) {
    if(entry.isFile) {
      if(entry.file)
        entry.file(function(f) {
          upload_file(base_url + entry.fullPath.split("/").filter(function(seg) { return seg; }).map(encodeURIComponent).join("/"), f);
        });
      else
        upload_file(base_url + entry.fullPath.split("/").filter(function(seg) { return seg; }).map(encodeURIComponent).join("/"), entry.getFile());
    } else // https://developer.mozilla.org/en-US/docs/Web/API/DataTransferItem/webkitGetAsEntry#javascript:
           //   Note: To read all files in a directory, readEntries needs to be
           //   called repeatedly until it returns an empty array. In
           //   Chromium-based browsers, the following example will only return a
           //   max of 100 entries.
           // This is actually true.
      all_in_reader(entry.createReader(), function(f) {
        recurse_upload(f, base_url)
      });
  }

  function all_in_reader(reader, f) {
    reader.readEntries(function(e) {
      e.forEach(f);
      if(e.length)
        all_in_reader(reader, f);
    });
  }
});
