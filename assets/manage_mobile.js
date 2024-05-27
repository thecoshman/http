"use strict";

window.addEventListener("DOMContentLoaded", function() {
  let first_onclick = true, input;
  let submit_callback = function() {
    if(first_onclick) {
      first_onclick = false;
      create_new_directory(input.value, new_directory.firstChild);
    }
  };

  new_directory.onclick = function(ev) {
    ev.preventDefault();

    if(!input) {
      make_confirm_icon(new_directory.firstChild, submit_callback);
      input = make_filename_input(document.createElement("span"), "", submit_callback);
      new_directory.appendChild(input.parentElement);
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
