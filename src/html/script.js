var cmds = [];

document.body.style.margin = '0';
document.body.style.overflow = 'hidden';

var canvas = document.getElementById('canvas');
// TODO: Switch to OffscreenCanvas once it supports fillText
//var offscreen = canvas.transferControlToOffscreen();
var ctx = canvas.getContext("2d", {"alpha": false});

function drawCommand(c) {
  window[c.type](c);
}

function draw() {
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = '#eee';
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = '#000';
  ctx.save();
  cmds.forEach(drawCommand);
  ctx.restore();
  socket.send(JSON.stringify({
    "type": "render_done",
    "time": performance.now()
  }));

  requestAnimationFrame(function() {
    if (socket.readyState != 1) return;
    socket.send(JSON.stringify({
      "type": "render_ready",
      "time": performance.now()
    }));
  });
}

function translate(cmd) { ctx.translate(cmd.x, cmd.y); }
function fillText(cmd) { ctx.fillText(cmd.text, cmd.x, cmd.y); }
function fillRect(cmd) { ctx.fillRect(cmd.x, cmd.y, cmd.w, cmd.h); }
function rect(cmd) { ctx.rect(cmd.x, cmd.y, cmd.w, cmd.h); }
function arc(cmd) { ctx.arc(cmd.x, cmd.y, cmd.r, cmd.alpha, cmd.beta); }
function moveTo(cmd) { ctx.moveTo(cmd.x, cmd.y); }
function lineTo(cmd) { ctx.lineTo(cmd.x, cmd.y); }
function setLineDash(cmd) { ctx.setLineDash(cmd.val); }
function rotate(cmd) { ctx.rotate(cmd.val); }
function scale(cmd) { ctx.scale(cmd.val, cmd.val); }

function fillStyle(cmd) { ctx.fillStyle = cmd.val; }
function textAlign(cmd) { ctx.textAlign = cmd.val; }
function textBaseline(cmd) { ctx.textBaseline = cmd.val; }
function lineWidth(cmd) { ctx.lineWidth = cmd.val; }
function strokeStyle(cmd) { ctx.strokeStyle = cmd.val; }
function font(cmd) { ctx.font = cmd.val; }

function save() { ctx.save(); }
function restore() { ctx.restore(); }
function beginPath() { ctx.beginPath(); }
function closePath() { ctx.closePath(); }
function fill() { ctx.fill(); }
function stroke() { ctx.stroke(); }
function clip() { ctx.clip(); }

var socket = undefined;
var binds = [
  {"html": "onmousedown", "mvm": "mouse_down", "x": "clientX", "y": "clientY", "button": "button"},
  {"html": "onmousemove", "mvm": "mouse_move", "x": "clientX", "y": "clientY"},
  {"html": "onmouseup",   "mvm": "mouse_up",   "x": "clientX", "y": "clientY", "button": "button"},
  {"html": "onwheel",     "mvm": "wheel",     "x": "deltaX",  "y": "deltaY"},
  {"html": "onkeydown",   "mvm": "key_down",   "code": "code", "key": "key"},
  {"html": "onkeyup",     "mvm": "key_up",     "code": "code", "key": "key"},
  {"html": "oncontextmenu"},
];

function SocketMessage(e) {
  var msg = JSON.parse(e.data);
  if (Array.isArray(msg)) {
    cmds = msg;
    draw();
  } else {
    if (msg.type === "measureText") {
      var w = ctx.measureText(msg.text).width;
      socket.send(JSON.stringify({
	"type": "textWidth",
	"width": w
      }));
    }
  }
};

function Reconnect() {
  ctx.fillStyle = 'white';
  ctx.fillRect(0,0,canvas.width, canvas.height);
  ctx.fillStyle = 'black';
  ctx.save();
  ctx.textAlign = "center";
  ctx.translate(canvas.width/2, canvas.height/2);
  ctx.fillText("Stopped", 0, 0);
  ctx.restore();
  setTimeout(Connect, 1000);
};

function Connect() {
  socket = new WebSocket("ws://localhost:8081/");
  socket.onmessage = SocketMessage;
  socket.onopen = SocketOpen;
  socket.onerror = Reconnect;
};

function SocketClose() {
  window.onresize = undefined;
  binds.forEach(function(bind) { window[bind.html] = undefined; });
  Reconnect();
};

function WindowResize(e) {
  socket.send(JSON.stringify({
    "type": "size",
    "width": innerWidth,
    "height": innerHeight
  }));
  canvas.width = innerWidth;
  canvas.height = innerHeight;
  ctx.font = '20px Iosevka';
  draw();
};

function Bind(bind) {
  window[bind.html] = function(e) {
    if (typeof bind.mvm != "undefined") {
      var o = { "type": bind.mvm };
      for (var key in bind) {
	if (key == "html" || key == "mvm") continue;
	o[key] = e[bind[key]];
      }
      socket.send(JSON.stringify(o));
    }
    e.preventDefault();
    return true;
  }
};

function SocketOpen(e) {
  socket.onerror = undefined;
  window.onresize = WindowResize;
  window.onresize();
  binds.forEach(Bind);
  socket.onclose = SocketClose;
};

Connect();
