window.addEventListener("load", function() {
  let new_directory_line = document.getElementById("new_directory");
  let new_directory_filename_cell = new_directory_line.children[1];
  let new_directory_status_output = new_directory_line.children[4].children[0];

  let new_directory_filename_input = null;

  let delete_file_links = document.getElementsByClassName("delete_file_icon");


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
          create_new_directory(new_directory_filename_input.value, new_directory_status_output);
      });

      new_directory_filename_input.focus();
    }
  });

  for(let i = delete_file_links.length - 1; i >= 0; --i) {
    let link = delete_file_links[i];

    link.addEventListener("click", function(ev) {
      ev.preventDefault();

      let line = link.parentElement.parentElement;
      make_request("DELETE", line.children[0].children[0].href, link);
    });
  }


  function create_new_directory(fname, status_out) {
    let req_url = window.location.origin + window.location.pathname;
    if(!req_url.endsWith("/"))
      req_url += "/";
    req_url += encodeURIComponent(fname);

    make_request("MKCOL", req_url, status_out);
  };

  function make_request(verb, url, status_out) {
    let request = new XMLHttpRequest();
    request.addEventListener("loadend", function(ev) {
      if(request.status >= 200 && request.status < 300)
        window.location.reload();
      else {
        status_out.innerHTML = request.status + " " + request.statusText + " — " + request.response;
        status_out.classList.add("has-log");
      }
    });
    request.open(verb, url);
    request.send();
  };
});
