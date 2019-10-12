import datetime
import http.server
import minishogilib
import numpy as np
from optparse import OptionParser
import _pickle
import queue
import simplejson
import socketserver
import sys
import tensorflow as tf
import tensorflow.keras.backend as K
import threading
import time

import mcts
import network
from reservoir import Reservoir


class Trainer():
    def __init__(self, port, store_only=False, record_file=None, weight_file=None):
        self.port = port

        self.reservoir = Reservoir()
        self.nn = network.Network(False)

        self.steps = 0

        self.weights = _pickle.dumps(self.nn.get_weights(), protocol=4)
        self.nn_lock = threading.Lock()

        self.store_only = store_only

        if not record_file is None:
            self.reservoir.load(record_file)

        if not weight_file is None:
            self.nn.load(weight_file)
        else:
            self.nn.save('./weights/iter_0.h5')

        self.training_data = queue.Queue(maxsize=10)

    def _sample_datasets(self):
        BATCH_SIZE = 4096
        RECENT_GAMES = 100000

        while True:
            if self.reservoir.len_learning_targets() < BATCH_SIZE:
                continue

            datasets = self.reservoir.sample(self.nn, BATCH_SIZE, RECENT_GAMES)
            self.training_data.put(datasets)

    def collect_records(self):
        print('Ready')
        log_file = open('connection_log.txt', 'w')

        weights = self.weights
        nn_lock = self.nn_lock
        reservoir = self.reservoir

        class handler(http.server.SimpleHTTPRequestHandler):
            def do_GET(self):
                if self.path == '/weight':
                    self.send_response(200)
                    self.send_header('Content-type', 'text/html')
                    self.end_headers()

                    with nn_lock:
                        self.wfile.write(weights)

                    log_file.write('[{}] send the parameters\n'.format(datetime.datetime.now(datetime.timezone.utc)))
                    log_file.flush()

                else:
                    self.send_response(400)
                    self.send_header('Content-type', 'text/html')
                    self.end_headers()

            def do_POST(self):
                if self.path == '/record':
                    content_length = int(self.headers.get('content-length'))
                    game_record = _pickle.loads(self.rfile.read(content_length))
                    reservoir.push(game_record)

                    self.send_response(200)
                    self.send_header('Content-type', 'text/html')
                    self.end_headers()

                    log_file.write('[{}] received a game record\n'.format(datetime.datetime.now(datetime.timezone.utc)))
                    log_file.flush()
                else:
                    self.send_response(400)
                    self.send_header('Content-type', 'text/html')
                    self.end_headers()

        class ThreadedHTTPServer(socketserver.ThreadingMixIn, socketserver.TCPServer):
            pass

        with ThreadedHTTPServer(('', self.port), handler) as httpd:
            httpd.serve_forever()

    def update_parameters(self):
        sample_thread = threading.Thread(target=self._sample_datasets)
        sample_thread.start()

        log_file = open('training_log.txt', 'w')

        position = minishogilib.Position()
        position.set_start_position()
        init_position_nn_input = self.nn.get_inputs([position])

        while True:
            nninputs, policies, values = self.training_data.get()

            # Update neural network parameters
            if self.steps < 100000:
                learning_rate = 1e-1
            elif self.steps < 300000:
                learning_rate = 1e-2
            elif self.steps < 500000:
                learning_rate = 1e-3
            else:
                learning_rate = 1e-4

            loss = self.nn.step(
                nninputs, policies, values, learning_rate)
            init_policy, init_value = self.nn.predict(
                init_position_nn_input)

            if self.steps % 5000 == 0:
                self.nn.save('./weights/iter_{}.h5'.format(self.steps))

            with self.nn_lock:
                self.weights = _pickle.dumps(self.nn.get_weights(), protocol=4)

            log_file.write('{}, {}, {}, {}, {}, {}\n'.format(datetime.datetime.now(
                datetime.timezone.utc), self.steps, loss['loss'], loss['policy_loss'], loss['value_loss'], init_value[0][0]))
            log_file.flush()

            self.steps += 1

    def run(self):
        # Make the server which receives game records by selfplay from clients
        collect_records_thread = threading.Thread(target=self.collect_records)
        collect_records_thread.start()

        # Update the neural network parameters
        if not self.store_only:
            self.update_parameters()


if __name__ == '__main__':
    parser = OptionParser()
    parser.add_option('-p', '--port', dest='port', type='int',
                      default=10055, help='port')
    parser.add_option('-s', '--store', action='store_true', dest='store', default=False,
                      help='Only store game records. Training will not be conducted.',)
    parser.add_option('-r', '--record_file', dest='record_file',
                      default=None, help='Game records already played')
    parser.add_option('-w', '--weight_file', dest='weight_file',
                      default=None, help='Weights of neural network parameters')

    (options, args) = parser.parse_args()

    trainer = Trainer(options.port, options.store,
                      options.record_file, options.weight_file)
    trainer.run()
