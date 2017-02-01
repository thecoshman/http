window.addEventListener("load", () => {
  const FORMAT = "yyyy-MM-dd HH:mm:ss";

  let timestamps = document.getElementsByClassName("datetime");
  Array.from(timestamps).forEach((r) => {
    let dt = r.innerText.replace("UTC", "").trim();
    r.innerText = Date.parseString(dt, FORMAT).format(FORMAT);
  });
});
