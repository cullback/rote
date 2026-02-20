(function () {
  var back = document.getElementById("back-section");
  var btn = document.getElementById("reveal-btn");
  var hint = document.getElementById("reveal-hint");
  var form = document.getElementById("grade-form");
  if (!back || !btn) return;

  var revealed = false;

  function reveal() {
    if (revealed) return;
    revealed = true;
    back.style.display = "";
    btn.style.display = "none";
    if (hint) hint.style.display = "none";
    if (form) form.style.display = "";
  }

  function grade(n) {
    if (!revealed || !form) return;
    form.querySelector('input[name="grade"]').value = n;
    form.submit();
  }

  btn.addEventListener("click", reveal);

  document.addEventListener("keydown", function (e) {
    var t = e.target.tagName;
    if (t === "INPUT" || t === "TEXTAREA" || t === "SELECT") return;
    if (e.key === " ") { e.preventDefault(); reveal(); }
    else if (e.key >= "1" && e.key <= "4") { grade(e.key); }
  });
})();
