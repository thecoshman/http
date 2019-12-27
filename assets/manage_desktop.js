"use strict";

window.addEventListener("load", function() {
  let new_directory_line = document.getElementById("new_directory");

  if(new_directory_line) {
    let new_directory_filename_cell = new_directory_line.children[1];
    let new_directory_status_output = new_directory_line.children[4].children[0];
    let new_directory_filename_input = null;

    new_directory_line.addEventListener("click", function(ev) {
      if(new_directory_filename_input === null)
        ev.preventDefault();
      else if(ev.target === new_directory_status_output)
        ;
      else if(ev.target !== new_directory_filename_input) {
        ev.preventDefault();
        new_directory_filename_input.focus();
      }

      if(new_directory_filename_input === null) {
        let submit_callback = function() {
          create_new_directory(new_directory_filename_input.value, new_directory_status_output);
        };

        new_directory_filename_input = make_filename_input(new_directory_filename_cell, "", submit_callback);
        make_confirm_icon(new_directory_status_output, submit_callback);
      }
    }, true);
  }
});


function get_href_for_line(line) {
  return line.children[0].children[0].href;
}

function get_filename_cell_for_line(line) {
  return line.children[1];
}
