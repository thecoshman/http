window.addEventListener("load", function() {
  let new_directory_line = document.getElementById("new_directory");
  let new_directory_filename_cell = new_directory_line.children[1];
  let new_directory_status_output = new_directory_line.children[4].children[0];

  let new_directory_filename_input = null;


  new_directory_line.addEventListener("click", function(ev) {
    if(new_directory_filename_input === null)
      ev.preventDefault();
    else if(ev.target !== new_directory_filename_input) {
      ev.preventDefault();
      new_directory_filename_input.focus();
    }

    if(new_directory_filename_input === null) {
      new_directory_filename_cell.innerHTML = "<input type=\"text\"></input>";
      new_directory_filename_input = new_directory_filename_cell.children[0];

      new_directory_filename_input.addEventListener("keypress", function(ev) {
        if(ev.keyCode === 13)  // Enter
          create_new_directory(new_directory_filename_input.value);
      });

      new_directory_filename_input.focus();
    }
  });


  function create_new_directory(fname, status_out) {
    let req_url = window.location.origin + window.location.pathname;
    if(!req_url.endsWith("/"))
      req_url += "/";
    req_url += encodeURIComponent(fname);

    let request = new XMLHttpRequest();
    request.addEventListener("loadend", function(ev) {
      if(request.status >= 200 && request.status < 300)
        window.location.reload();
      else
        status_out.innerHTML = request.status + " " + request.statusText + " — " + request.response;
    });
    request.open("MKCOL", req_url);
    request.send();
  };
});
