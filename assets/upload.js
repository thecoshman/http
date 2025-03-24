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

      for(let i = ev.dataTransfer.files.length - 1; i >= 0; --i) {
        if(!ev.dataTransfer.items[i].webkitGetAsEntry) {
          let file = ev.dataTransfer.files[i];
          upload_file(url + encodeURIComponent(file.name), file);
        } else
          recurse_upload(ev.dataTransfer.items[i].webkitGetAsEntry(), url);
      }
    }
  });

  let file_upload = document.querySelector("input[type=file]");
  let upload_list = document.createElement('ol');
  file_upload.parentNode.appendChild(upload_list)

  file_upload.addEventListener("change", function() {
    for(let i = file_upload.files.length - 1; i >= 0; --i) {
      let file = file_upload.files[i];
      upload_file(url + encodeURIComponent(file.name), file);
    }
  });

  function upload_file(req_url, file) {
    ++remaining_files;

    let list_item = document.createElement('li');
    let filename_line = document.createElement('p')
    let progress_line = document.createElement('p')

    filename_line.textContent = file.name

    list_item.appendChild(filename_line)
    list_item.appendChild(progress_line)
    upload_list.appendChild(list_item)

    if(!file_upload_text) {
      file_upload_text = document.createTextNode(1);
      file_upload.parentNode.insertBefore(file_upload_text, file_upload.nextSibling); // insertafter
    } else
      file_upload_text.data = remaining_files;

    let request = new XMLHttpRequest();
    let startTime = Date.now();

    request.addEventListener("loadend", function(e) {
      if(request.status >= 200 && request.status < 300) {
        progress_line.textContent = 'Done'
        if(!--remaining_files)
         window.location.reload();
        file_upload_text.data = remaining_files;
      } else {
        progress_line.textContent = request.response;
        file_upload.outerHTML = req_url + "<br />" + request.response;
      }
    });

    request.upload.addEventListener("progress", function(e) {
      if (e.lengthComputable) {
        const total = event.total;
        const loaded = event.loaded;

        const percentComplete = (loaded / total) * 100;
        const elapsedTime = Math.floor((Date.now() - startTime) / 1000); // in seconds
        const speed = (loaded / elapsedTime) || 0; // bytes per second
        const totalTime = Math.floor(total / speed) || 0; // in seconds

        // Print output
        progress_line.textContent = progressString(
          percentComplete.toFixed(0),
          formatBytes(loaded),
          formatBytes(total),
          formatTime(elapsedTime),
          formatTime(totalTime),
          formatBytes(speed) + "/s",
        );
      }
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

  // Helper function to format display
  function progressString(percent, loaded, total, spent, all, speed) {
    return `${percent}% ${speed} ${loaded}/${total} ${spent}/${all}`;
  }

  // Function to format bytes to a human-readable string
  function formatBytes(bytes) {
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let i = 0;
    while (bytes >= 1024 && i < units.length - 1) {
        bytes /= 1024;
        i++;
    }
    return `${bytes.toFixed(2)} ${units[i]}`;
  }

  // Function to format seconds to a human-readable time string
  function formatTime(seconds) {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const remainingSeconds = seconds % 60;

    if (hours > 0) {
        return `${hours}h ${minutes}m ${remainingSeconds}s`;
    } else if (minutes > 0) {
        return `${minutes}m ${remainingSeconds}s`;
    } else {
        return `${remainingSeconds}s`;
    }
  }
});
