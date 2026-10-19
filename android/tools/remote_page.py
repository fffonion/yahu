"""Firefox remote-debugger helpers for local ADB UI checks."""
import json
import socket
import subprocess


class RemotePage:
    def __init__(self, serial=None, package='io.github.fffonion.yahu'):
        self.serial = serial
        self.port = int(self.adb('forward', 'tcp:0', 'localabstract:' + package + '/firefox-debugger-socket'))
        try:
            self.socket = socket.create_connection(('127.0.0.1', self.port), timeout=15)
            self._read()
            self.refresh()
        except BaseException:
            self.adb('forward', '--remove', 'tcp:' + str(self.port))
            raise

    def adb(self, *args):
        command = ['adb']
        if self.serial is not None:
            command.extend(['-s', self.serial])
        return subprocess.check_output([*command, *args], text=True).strip()

    def _read(self):
        header = b''
        while not header.endswith(b':'):
            part = self.socket.recv(1)
            if not part:
                raise ConnectionError('Remote debugger disconnected')
            header += part
        size = int(header[:-1])
        if size > 8 * 1024 * 1024:
            raise ValueError('Remote debugger packet exceeds the diagnostic limit')
        body = b''
        while len(body) < size:
            part = self.socket.recv(size - len(body))
            if not part:
                raise ConnectionError('Remote debugger disconnected')
            body += part
        return json.loads(body)

    def _send(self, payload):
        body = json.dumps(payload, separators=(',', ':')).encode()
        self.socket.sendall(str(len(body)).encode() + b':' + body)

    def _request(self, payload):
        self._send(payload)
        while True:
            response = self._read()
            if response.get('from') == payload['to'] and 'type' not in response:
                if response.get('error'):
                    raise RuntimeError('Remote debugger request failed')
                return response

    def refresh(self):
        tabs = self._request({'to': 'root', 'type': 'listTabs'})['tabs']
        for tab in tabs:
            target = self._request({'to': tab['actor'], 'type': 'getTarget'})['frame']
            self.console = target['consoleActor']
            if self.evaluate('document.visibilityState') == 'visible':
                return
        raise RuntimeError('Bring the Yahu Activity to the foreground before inspecting it')

    def evaluate(self, expression):
        self._send({'to': self.console, 'type': 'evaluateJSAsync', 'text': expression, 'options': {}})
        while True:
            response = self._read()
            if response.get('from') != self.console:
                continue
            if response.get('error'):
                raise RuntimeError('Remote page evaluation unavailable')
            if response.get('type') == 'evaluationResult':
                if response.get('exception'):
                    raise RuntimeError('Remote page evaluation failed')
                return response.get('result')

    def data(self, expression):
        return json.loads(self.evaluate('JSON.stringify(' + expression + ')'))

    def close(self):
        self.socket.close()
        self.adb('forward', '--remove', 'tcp:' + str(self.port))

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
