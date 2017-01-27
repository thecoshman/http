window.addEventListener("load", () => {
  const SUPPORTED_TYPES = ["Files", "application/x-moz-file"];

  let body = document.getElementsByTagName("body")[0];

  body.addEventListener("dragover", (ev) => {
    if(SUPPORTED_TYPES.find(el => (ev.dataTransfer.types.contains || ev.dataTransfer.types.includes).call(ev.dataTransfer.types, el)))
      ev.preventDefault();
  });

  body.addEventListener("drop", (ev) => {
    if(SUPPORTED_TYPES.find(el => (ev.dataTransfer.types.contains || ev.dataTransfer.types.includes).call(ev.dataTransfer.types, el))) {
      ev.preventDefault();

      let remaining_files = ev.dataTransfer.files.length;
      let url = document.URL;
      if(url[url.length - 1] != "/")
        url += "/";

      for(let i = ev.dataTransfer.files.length - 1; i >= 0; --i) {
        let file = ev.dataTransfer.files[i];
        let request = new XMLHttpRequest();
        request.addEventListener("loadend", (e) => {
          if(--remaining_files === 0)
            window.location.reload();
        });
        request.open("PUT", url + file.name);
        request.send(file);
      }
    }
  });
});
