"use strict";

window.addEventListener("load", function() {
  let delete_file_links = document.getElementsByClassName("delete_file_icon");
  let rename_links = document.getElementsByClassName("rename_icon");


  for(let i = delete_file_links.length - 1; i >= 0; --i) {
    let link = delete_file_links[i];

    link.addEventListener("click", function(ev) {
      ev.preventDefault();

      let line = link.parentElement.parentElement;
      make_request("DELETE", get_href_for_line(line), link);
    });
  }

  let first_rename_onclick = function(link, first_onclick, ev) {
    ev.preventDefault();

    let line = link.parentElement.parentElement;
    let filename_cell = get_filename_cell_for_line(line);
    let original_name = filename_cell.innerText;

    let submit_callback = function() {
      rename(original_name, new_name_input.value, link);
    };
    let new_name_input = make_filename_input(filename_cell, original_name, submit_callback);

    link.removeEventListener("click", first_onclick);
    make_confirm_icon(link, submit_callback);
  };
  for(let i = rename_links.length - 1; i >= 0; --i) {
    let link = rename_links[i];

    let first_onclick = function(ev) {
      first_rename_onclick(link, first_onclick, ev);
    };
    link.addEventListener("click", first_onclick);
  }


  function rename(fname_from, fname_to, status_out) {
    let root_url = window.location.origin + window.location.pathname;
    if(!root_url.endsWith("/"))
      root_url += "/";

    if(fname_from.endsWith("/"))
      fname_from = fname_from.substr(0, fname_from.length - 1);
    if(fname_to.endsWith("/"))
      fname_to = fname_to.substr(0, fname_to.length - 1);

    make_request("MOVE", root_url + encodeURI(fname_from), status_out, function(request) {
      request.setRequestHeader("Destination", root_url + encodeURI(fname_to));
    });
  };
});


function make_filename_input(input_container, initial, callback) {
  input_container.innerHTML = "<input type=\"text\"></input>";
  let input_elem = input_container.children[0];
  input_elem.value = initial;

  input_elem.addEventListener("keypress", function(ev) {
    if(ev.keyCode === 13) {  // Enter
      ev.preventDefault();
      callback();
    }
  });
  input_container.addEventListener("click", function(ev) {
    ev.preventDefault();
  });

  input_elem.focus();

  return input_elem;
};

function make_confirm_icon(element, callback) {
  element.classList.add("confirm_icon");
  element.href = "#confirm";
  element.innerText = "Confirm";
  element.addEventListener("click", function(ev) {
    ev.preventDefault();
    callback();
  });
};

function create_new_directory(fname, status_out) {
  let req_url = window.location.origin + window.location.pathname;
  if(!req_url.endsWith("/"))
    req_url += "/";
  req_url += encodeURI(fname);

  make_request("MKCOL", req_url, status_out);
};

function make_request(verb, url, status_out, request_modifier) {
  let request = new XMLHttpRequest();
  request.addEventListener("loadend", function() {
    if(request.status >= 200 && request.status < 300)
      window.location.reload();
    else {
      status_out.innerHTML = request.status + " " + request.statusText + (request.response ? " — " : "") + request.response;
      status_out.classList.add("has-log");
    }
  });
  request.open(verb, url);
  if(request_modifier)
    request_modifier(request);
  request.send();
};
