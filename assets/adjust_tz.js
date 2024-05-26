"use strict";

window.addEventListener("DOMContentLoaded", function() {
  let modtime_h = document.getElementsByTagName("th")[2];
  if(modtime_h)
    modtime_h.innerText = modtime_h.innerText.replace(" (UTC)", "");

  let timestamps = document.getElementsByTagName("time");
  for(let r of timestamps) {
    let dt = new Date(parseInt(r.getAttribute("ms")));
    dt.setMinutes(dt.getMinutes() - dt.getTimezoneOffset())
    r.innerText = dt.toISOString().slice(0, 19).replace("T", " ");
  }
});
