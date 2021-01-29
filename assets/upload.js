"use strict";

window.addEventListener("load", function() {
  const SUPPORTED_TYPES = ["Files", "application/x-moz-file"];

  let body = document.querySelector("body");
  let file_upload = document.querySelector("#file_upload");
  let remaining_files = 0;
  let page_path = document.location.pathname.replace(/\/+$/, "") + "/"; // trim all trailing '/'s then put one back

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
        if(!ev.dataTransfer.items[i].webkitGetAsEntry)
          ++remaining_files;
        else
          recurse_count(ev.dataTransfer.items[i].webkitGetAsEntry());
      }

      for(let i = ev.dataTransfer.files.length - 1; i >= 0; --i) {
        if(!ev.dataTransfer.items[i].webkitGetAsEntry) {
          let file = ev.dataTransfer.files[i];
          upload_file(page_path + encodeURIComponent(file.name), file);
        } else
          recurse_upload(ev.dataTransfer.items[i].webkitGetAsEntry(), page_path);
      }
    }
  });

  file_upload.addEventListener("change", function() {
    remaining_files += file_upload.files.length;

    for(let i = file_upload.files.length - 1; i >= 0; --i) {
      let file = file_upload.files[i];
      upload_file(page_path + encodeURIComponent(file.name), file);
    }
  });

  function upload_file(req_url, file) {
    let request = new XMLHttpRequest();
    request.addEventListener("loadend", function(e) {
      if(--remaining_files === 0)
        window.location.reload();
    });
    request.open("PUT", req_url);
    request.send(file);
  }

  function recurse_upload(entry, base_path) {
    if(entry.isFile) {
      entry.file(function(f) {
        let file_path = entry.fullPath.replace(/^\/+/, ""); // we don't want the leading '/'s
        // each path segment needs to be individually encoded
        // e.g. encodeURIComponent("some#/$path") == "some%23%2F%24path" which is wrong
        //      we want "some%23/%24path" instead
        file_path = file_path.split("/").map((seg) => encodeURIComponent(seg)).join("/");
        upload_file(base_path + file_path, f);
      });
    } else
      entry.createReader().readEntries(function(e_arr) {
        e_arr.forEach(function(f) {
          recurse_upload(f, base_path)
        });
      });
  }

  function recurse_count(entry) {
    if(entry.isFile) {
      ++remaining_files;
    } else
      entry.createReader().readEntries(function(e) {
        e.forEach(recurse_count);
      });
  }
});
