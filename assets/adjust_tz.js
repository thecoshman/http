window.addEventListener("load", function() {
  const FORMAT = "yyyy-MM-dd HH:mm:ss";

  let modtime_h = document.getElementsByTagName("th")[2];
  if(modtime_h)
    modtime_h.innerText = modtime_h.innerText.replace("(UTC)", "").trim();

  let timestamps = document.getElementsByClassName("datetime");
  Array.from(timestamps).forEach(function(r) {
    let dt = r.innerText.replace("UTC", "").trim();
    r.innerText = Date.parseString(dt, FORMAT).format(FORMAT)
  });
});
