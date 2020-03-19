import datetime
from flask import Flask, render_template
from flask_socketio import SocketIO
import math
import queue
import simplejson as json
import subprocess
import threading
import time

import minishogilib


class Engine():
    def __init__(self, command=None, cwd=None, verbose=False, usi_option={}, timelimit={}):
        self.verbose = verbose
        self.command = command
        self.usi_option = usi_option
        self.timelimit = timelimit
        self.socketio = None

        self.time_left = 0
        self.byoyomi = 0

        self.process = subprocess.Popen(command.split(
        ), cwd=cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        self.message_queue = queue.Queue()
        threading.Thread(target=self._message_reader).start()

    def set_socketio(self, socketio):
        self.socketio = socketio

    def _message_reader(self):
        """Receive message from the engine through standard output and store it.
        # Arguments
            verbose: If true, print message in stdout.
        """
        with self.process.stdout:
            for line in iter(self.process.stdout.readline, b''):
                message = line.decode('utf-8').rstrip('\r\n')
                self.message_queue.put(message)

                if self.verbose:
                    if  self.socketio is not None:
                        self.socketio.emit('message', message, broadcast=True)
                    print('<:', message)

    def send_message(self, message):
        """Send message to the engine through standard input.
        # Arguments
            message: message sent to the engine.
            verbose: If true, print message in stdout.
        """
        if self.verbose:
            print('>:', message)

        message = (message + '\n').encode('utf-8')
        self.process.stdin.write(message)
        self.process.stdin.flush()

    def readline(self):
        message = self.message_queue.get()
        return message

    def ask_nextmove(self, position):
        sfen_position = 'position sfen ' + position.sfen(True)
        command = 'go {} {} {} {} {} {}'.format(
            self.timelimit['btime'], self.time_left,
            self.timelimit['wtime'], self.time_left,
            self.timelimit['byoyomi'], self.byoyomi)

        self.send_message(sfen_position)
        self.send_message(command)

        while True:
            line = self.readline().split()

            if line[0] == 'bestmove':
                return line[1]

    def usi(self):
        self.send_message('usi')

        while True:
            line = self.readline()

            if line == 'usiok':
                break

    def isready(self):
        for (key, value) in self.usi_option.items():
            command = 'setoption name {} value {}'.format(key, value)
            self.send_message(command)

        self.send_message('isready')

        while True:
            line = self.readline()

            if line == 'readyok':
                break

    def usinewgame(self):
        self.send_message('usinewgame')

    def quit(self):
        self.send_message('quit')

def dump_csa(position):
    data = []

    data.append('V2.2')
    data.append('N+SENTE')
    data.append('N-GOTE')
    data.append('P1-HI-KA-GI-KI-OU')
    data.append('P2 *  *  *  * -FU')
    data.append('P3 *  *  *  *  * ')
    data.append('P4+FU *  *  *  * ')
    data.append('P5+OU+KI+GI+KA+HI')
    data.append('+')

    csa_kif = position.get_csa_kif()

    for (ply, kif) in enumerate(csa_kif):
        if ply % 2 == 0:
            data.append('+{}'.format(kif))
        else:
            data.append('-{}'.format(kif))
        data.append('T0')

    return '\n'.join(data)

def main():
    app = Flask(__name__, template_folder='./')
    app.debug = False
    socketio = SocketIO(app)

    position = minishogilib.Position()
    position.set_start_position()

    consumptions = []

    with open('settings.json') as f:
        settings = json.load(f)

    engine = Engine(**settings['engine'])
    engine.set_socketio(socketio)
    engine.usi()
    engine.isready()

    @app.route('/')
    def sessions():
        return render_template('index.html')

    @socketio.on('display')
    def display():
        data = {
            'svg': position.to_svg(),
            'kif': position.get_csa_kif(),
            'timelimit': engine.time_left,
            'byoyomi': engine.byoyomi
        }

        socketio.emit('display', data, broadcast=True)

    @socketio.on('download')
    def download():
        current_time = '{0:%Y-%m-%d-%H%M%S}'.format(datetime.datetime.now())

        data = {
            'kif': dump_csa(position),
            'filename': '{}.csa'.format(current_time)
        }

        return data, 200

    @socketio.on('command')
    def command(data):
        data = data.split(' ')
        if data[0] == 'newgame':
            position.set_start_position()
            engine.usinewgame()

            engine.time_left = settings['timelimit']
            engine.byoyomi = settings['byoyomi']

            display()

        elif data[0] == 'move':
            if len(data) < 2:
                socketio.emit('message', 'You have to specify the next move.', broadcast=True)
                return

            move = data[1]
            moves = position.generate_moves()
            moves_sfen = [m.sfen() for m in moves]

            if not move in moves_sfen:
                socketio.emit('message', '{} is not a legal move.'.format(move), broadcast=True)
                return

            move = position.sfen_to_move(move)
            position.do_move(move)

            consumptions.append(0)

            display()

        elif data[0] == 'undo':
            if position.get_ply() == 0:
                socketio.emit('message', 'This is the initial position and you cannot go back more.', broadcast=True)
                return

            position.undo_move()
            last_consumption = consumptions.pop()
            engine.time_left += last_consumption

            display()

        elif data[0] == 'go':
            if engine.time_left == 0:
                socketio.emit('message', '0 seconds left to think.', broadcast=True)
                return

            current_time = time.time()
            next_move = engine.ask_nextmove(position)
            elapsed = time.time() - current_time
            elapsed = int(max(math.floor(elapsed), 1))
            elapsed = elapsed * 1000
            elapsed = min(engine.time_left, elapsed)
            engine.time_left -= elapsed
            consumptions.append(elapsed)

            if next_move != "resign":
                next_move = position.sfen_to_move(next_move)
                position.do_move(next_move)

            display()

        else:
            socketio.emit('message', 'Unknown command {}.'.format(data[0]), broadcast=True)


    socketio.run(app, host='0.0.0.0', port=8000)


if __name__ == '__main__':
    main()
