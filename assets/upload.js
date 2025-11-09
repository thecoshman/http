"use strict";

window.addEventListener("DOMContentLoaded", function() {
  const SUPPORTED_TYPES = ["Files", "application/x-moz-file"];

  let body = document.getElementsByTagName("body")[0];
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
      process_transfer(ev.dataTransfer);
    }
  });

  body.addEventListener("paste", function(ev) {
    if(ev.target.tagName != "INPUT" && (ev.clipboardData || window.clipboardData).files.length) {
      ev.preventDefault();
      process_transfer(ev.clipboardData || window.clipboardData);
    }
  });

  function process_transfer(transfer) {
    for(let i = transfer.files.length - 1; i >= 0; --i) {
      if(!transfer.items[i].webkitGetAsEntry) {
        let file = transfer.files[i];
        upload_file(url + encodeURIComponent(file.name), file);
      } else
        recurse_upload(transfer.items[i].webkitGetAsEntry(), url);
    }
  }

  let file_upload = document.querySelector("input[type=file]");
  let upload_list = document.createElement("dl");
  file_upload.parentElement.appendChild(upload_list);

  file_upload.addEventListener("change", function() {
    for(let i = file_upload.files.length - 1; i >= 0; --i) {
      let file = file_upload.files[i];
      upload_file(url + encodeURIComponent(file.name), file);
    }
  });

  function upload_file(req_url, file) {
    ++remaining_files;

    let filename_line = document.createElement("dt");
    filename_line.innerText = file.name;
    upload_list.appendChild(filename_line)

    let progress_progress_label = document.createElement("label");
    let progress_progress_desc  = document.createElement("span");
    let progress_progress       = document.createElement("progress");
    progress_progress.value = 0;
    progress_progress_label.appendChild(progress_progress);
    progress_progress_label.appendChild(document.createTextNode(" "));
    progress_progress_desc.innerText = "pending";
    progress_progress_label.appendChild(progress_progress_desc);

    let progress_speed = document.createElement("span");
    let progress_speed_text = document.createTextNode("?");
    progress_speed.appendChild(progress_speed_text);
    progress_speed.appendChild(document.createTextNode("/s"));

    let progress_time = document.createElement("span");
    progress_speed.appendChild(progress_time);

    let progress_line     = document.createElement("dd");
    progress_line.appendChild(progress_progress_label);
    progress_line.appendChild(document.createTextNode(" "));
    progress_line.appendChild(progress_speed);
    progress_line.appendChild(document.createTextNode(" "));
    progress_line.appendChild(progress_time);
    upload_list.appendChild(progress_line)

    if(!file_upload_text) {
      file_upload_text = document.createTextNode(1);
      file_upload.parentNode.insertBefore(file_upload_text, file_upload.nextSibling); // insertafter
    } else
      file_upload_text.data = remaining_files;

    let request = new XMLHttpRequest();
    request.addEventListener("loadend", function(e) {
      if(request.status >= 200 && request.status < 300) {
        if(!--remaining_files)
          window.location.reload();

        filename_line.remove();
        progress_line.remove();

        file_upload_text.data = remaining_files;
      } else {
        progress_line.innerText = request.response;
        file_upload.outerHTML = req_url + "<br />" + request.response;
      }
    });

    let start = 0;
    let update = -1000;
    request.upload.addEventListener("loadstart", function(e) {
      start = e.timeStamp;
    });
    let prog = function(e) {
      if(e.lengthComputable) {
        progress_progress.value = event.loaded;
        progress_progress.max   = event.total;

        let elapsed = (e.timeStamp - start) / 1000; // s
        if(elapsed > 0.1) {
          let speed = event.loaded / elapsed;
          progress_progress_desc.innerText = human_readable_size(event.loaded) + "/" + human_readable_size(event.total);
          progress_speed_text.data         = human_readable_size(speed);
          progress_time.innerText          = hms(elapsed) + "/" + hms(event.total / speed);
        }
      } else {
        progress_progress.removeAttribute("value");
        progress_progress_desc.innerText = "uploading";
        request.upload.removeEventListener("progress", prog);
      }
    };
    request.upload.addEventListener("progress", prog);

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

  // Ported from util::HumanReadableSize
  const LN_KIB = Math.LN2 * 10; // 1024f64.ln()
  function human_readable_size(num) {
    let exp = Math.min(Math.max(Math.trunc(Math.log(num) / LN_KIB), 0), 8);

    let val = num / Math.pow(2, exp * 10);

    return ((exp > 0) ? Math.round(val * 10) / 10 : Math.round(val)) + " "
           + ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB", "YiB"][Math.trunc(Math.max(exp, 0))];
  }

  function hms(seconds) {
    let h = Math.trunc(seconds / 3600);
    let m = Math.trunc((seconds % 3600) / 60);
    let s = Math.trunc(seconds % 60);

    if(h)
      return h + ":" + String(m).padStart(2, '0') + ":" + String(s).padStart(2, '0');
    else if(m)
      return m + ":" + String(s).padStart(2, '0');
    else
      return seconds.toFixed(1) + "s";
  }
});
