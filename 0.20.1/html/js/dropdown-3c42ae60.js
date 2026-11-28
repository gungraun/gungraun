/* When the user clicks on the button,
toggle between hiding and showing the dropdown content */
function versionDropdown() {
  document.getElementById("version-dropdown").classList.toggle("show");
}

// Close the dropdown menu if the user clicks outside of it
window.onclick = function (event) {
  if (!event.target.matches(".version-dropdown-button")) {
    var dropdowns = document.getElementsByClassName("version-dropdown-content");
    var i;
    for (i = 0; i < dropdowns.length; i++) {
      var openDropdown = dropdowns[i];
      if (openDropdown.classList.contains("show")) {
        openDropdown.classList.remove("show");
      }
    }
  }
};
