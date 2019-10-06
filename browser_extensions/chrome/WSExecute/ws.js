var last_pong = Date.now();
var socket = null;
var threshold = 10;
var ws_url = "ws://127.0.0.1:8001";

function check_alive() {
	var diff_s = (Date.now() - last_pong) / 1000;

	if (diff_s > threshold) {
		console.log("Last ping " + diff_s + " seconds ago, reopening connection");
		open_connection(ws_url);
	}
}

function open_connection(url) {
	if (socket != null) {
		socket.close();
		socket = null;
	}

	socket = new WebSocket(url);

	socket.addEventListener('open', function (event) {
	});

	socket.addEventListener('message', function (event) {
		var cmd = JSON.parse(event.data);

		if (cmd['event'] == 'welcome') {
			console.log("Connected to Websocket server " + ws_url + ", version: " + cmd['version']['main'] + "." + cmd['version']['patch']);
		
		} else if (cmd['event'] == 'data') {
			console.debug("Executing data: " + cmd['data']);
			chrome.tabs.executeScript({
			    "code": cmd['data'],
			});

		} else if (cmd['event'] == 'pong') {
			console.debug("Received PONG");
			last_pong = Date.now();

		} else {
			console.log("Unknown command from Websocket: ", cmd);
		}

	});
}

open_connection(ws_url);
setInterval(check_alive, 3000);
