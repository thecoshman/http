window.addEventListener("load", function() {
  const SUPPORTED_TYPES = ["Files", "application/x-moz-file"];

  let body = document.getElementsByTagName("body")[0];
  let file_upload = document.getElementById("file_upload");
  let remaining_files = 0;
  let url = document.URL;
  if(url[url.length - 1] == "/")
    url = url.substr(0, url.length - 1);

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
          upload_file(url + "/" + file.name, file);
        } else
          recurse_upload(ev.dataTransfer.items[i].webkitGetAsEntry(), url);
      }
    }
  });

  file_upload.addEventListener("change", function() {
    remaining_files += file_upload.files.length;

    for(let i = file_upload.files.length - 1; i >= 0; --i) {
      let file = file_upload.files[i];
      upload_file(url + "/" + file.name, file);
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

  function recurse_upload(entry, base_url) {
    if(entry.isFile) {
      if(entry.file)
        entry.file(function(f) {
          upload_file(base_url + entry.fullPath, f);
        });
      else
        upload_file(base_url + entry.fullPath, entry.getFile());
    } else
      entry.createReader().readEntries(function(e) {
        e.forEach(function(f) {
          recurse_upload(f, base_url)
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
