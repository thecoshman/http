window.addEventListener("load", function() {
  const FORMAT = "yyyy-MM-dd HH:mm:ss";

  let modtime_h = document.getElementsByTagName("th")[2];
  modtime_h.innerText = modtime_h.innerText.replace("(UTC)", "").trim();

  let timestamps = document.getElementsByClassName("datetime");
  Array.from(timestamps).forEach(function(r) {
    r.innerText = Date.parseString(r.innerText, FORMAT).format(FORMAT);
  });
});
