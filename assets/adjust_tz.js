"use strict";

window.addEventListener("load", function() {
  let modtime_h = document.getElementsByTagName("th")[2];
  if(modtime_h)
    modtime_h.innerText = modtime_h.innerText.replace(" (UTC)", "");

  let timestamps = document.getElementsByClassName("datetime");
  Array.from(timestamps).forEach(function(r) {
    let dt = new Date(r.innerText.replace(" UTC", "").replace(" ", "T") + "Z")
    dt.setMinutes(dt.getMinutes() - dt.getTimezoneOffset())
    r.innerText = dt.toISOString().replace("T", " ").replace(".000Z", " ");
  });
});
