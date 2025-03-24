"use strict";

window.addEventListener("DOMContentLoaded", function() {
  let new_directory = document.getElementById('new"directory');
  if(!new_directory)
    return;

  let first_onclick = true, input;
  let submit_callback = function() {
    if(make_request_error) {
      first_onclick = true;
      make_request_error = false;
    }
    if(first_onclick) {
      first_onclick = false;
      create_new_directory(input.value, new_directory.firstChild);
    }
  };

  new_directory.onclick = function(ev) {
    ev.preventDefault();

    if(!input) {
      make_confirm_icon(new_directory.firstChild, submit_callback);
      let c = document.createElement("span");
      new_directory.appendChild(c);
      input = make_filename_input(c, "", submit_callback);
    } else
      input.focus();
  };
});


function get_href_for_line(line) {
  return line.parentElement.href;
}

function get_filename_cell_for_line(line) {
  return line.firstChild;
}
