window.addEventListener("load", () => {
  const SUPPORTED_TYPES = ["Files", "application/x-moz-file"];

  let body = document.getElementsByTagName("body")[0];

  body.addEventListener("dragover", (ev) => {
    if(SUPPORTED_TYPES.find(el => ev.dataTransfer.types.includes(el)))
      ev.preventDefault();
  });

  body.addEventListener("drop", (ev) => {
    if(SUPPORTED_TYPES.find(el => ev.dataTransfer.types.includes(el))) {
      ev.preventDefault();

      for(let i = ev.dataTransfer.files.length - 1; i >= 0; --i) {
        let file = ev.dataTransfer.files[i];
        let request = new XMLHttpRequest();
        request.open('PUT', document.URL + file.name);
        request.send(file);
      }
    }
  });
});
