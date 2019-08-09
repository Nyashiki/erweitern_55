import datetime
import socket
from optparse import OptionParser
import pickle
import sys
import threading
import time

import mcts
import network
from reservoir import Reservoir
import tensorflow as tf
import tensorflow.keras.backend as K
import utils


class Trainer():
    def __init__(self, port, store_only=False, record_file=None, weight_file=None):
        self.port = port

        self.reservoir = Reservoir()
        self.nn = network.Network()
        self.nn.model._make_predict_function()
        self.session = tf.compat.v1.keras.backend.get_session()
        self.graph = tf.compat.v1.get_default_graph()

        self.steps = 0

        self.nn_lock = threading.Lock()
        self.reservoir_lock = threading.Lock()

        self.store_only = store_only

        if not record_file is None:
            self.reservoir.load(record_file)

        if not weight_file is None:
            self.nn.load(weight_file)

    def collect_records(self):
        sc = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sc.bind(('localhost', self.port))
        sc.listen(128)

        print('Ready')

        log_file = open('connection_log.txt', 'w')

        while True:
            conn, addr = sc.accept()
            message = conn.recv(1024)

            if message == b'parameter':
                with self.session.as_default():
                    with self.graph.as_default():
                        with self.nn_lock:
                            data = pickle.dumps(
                                self.nn.model.get_weights(), protocol=2)

                conn.send(len(data).to_bytes(16, 'little'))
                conn.sendall(data)

                data = conn.recv(16)
                assert data == b'parameter_ok', 'Protocol violation!'

                log_file.write('[{}] sent the parameters to {}\n'.format(
                    datetime.datetime.now(datetime.timezone.utc), str(addr)))

            elif message == b'record':
                conn.send(b'ready')

                data = conn.recv(16)
                data_size = int.from_bytes(data, 'little')
                data = utils.recvall(conn, data_size)
                game_record = pickle.loads(data)

                data = conn.recv(16)
                assert data == b'record_ok', 'Protocol violation!'

                print('-------------')
                print('[{}] readyok'.format(
                    datetime.datetime.now(datetime.timezone.utc)))

                with self.reservoir_lock:
                    print('[{}] lockok'.format(
                        datetime.datetime.now(datetime.timezone.utc)))
                    self.reservoir.push(game_record)
                    print('[{}] pushok'.format(
                        datetime.datetime.now(datetime.timezone.utc)))
                    print('reservoir_len', self.reservoir.len(),
                          self.reservoir.len_learning_targets())
                log_file.write('[{}] received a game record from {}\n'.format(
                    datetime.datetime.now(datetime.timezone.utc), str(addr)))

                with self.reservoir_lock:
                    if self.reservoir.len() % 5000 == 0:
                        print('[{}] save lockok'.format(
                            datetime.datetime.now(datetime.timezone.utc)))
                        self.reservoir.save('records.pkl')
                        print('[{}] save ok'.format(
                            datetime.datetime.now(datetime.timezone.utc)))
                print('-----------------------')
            log_file.flush()
            conn.close()

    def update_parameters(self):
        BATCH_SIZE = 1024
        RECENT_GAMES = 100000

        log_file = open('training_log.txt', 'w')

        while True:
            with self.reservoir_lock:
                if self.reservoir.len_learning_targets() < BATCH_SIZE:
                    continue

                nninputs, policies, values = self.reservoir.sample(
                    BATCH_SIZE, RECENT_GAMES)

            # Update neural network parameters
            with self.nn_lock:
                with self.session.as_default():
                    with self.graph.as_default():
                        if self.steps < 100000:
                            learning_rate = 1e-1
                        elif self.steps < 300000:
                            learning_rate = 1e-2
                        elif self.steps < 500000:
                            learning_rate = 1e-3
                        else:
                            learning_rate = 1e-4

                        loss_sum, policy_loss, value_loss = self.nn.step(
                            nninputs, policies, values, learning_rate)

                        if self.steps % 5000 == 0:
                            self.nn.model.save(
                                './weights/iter_{}.h5'.format(self.steps), include_optimizer=True)

            log_file.write('{}, {}, {}, {}, {}\n'.format(datetime.datetime.now(
                datetime.timezone.utc), self.steps, loss_sum, policy_loss, value_loss))
            log_file.flush()

            self.steps += 1

    def run(self):
        # Make the server which receives game records by selfplay from clients
        collect_records_thread = threading.Thread(target=self.collect_records)
        collect_records_thread.start()

        # Continue to update the neural network parameters
        update_parameters_thread = threading.Thread(
            target=self.update_parameters)
        if not self.store_only:
            update_parameters_thread.start()


if __name__ == '__main__':
    parser = OptionParser()
    parser.add_option('-p', '--port', dest='port', type='int',
                      default=10055, help='port')
    parser.add_option('-s', '--store', action='store_true', dest='store', default=False,
                      help='Only store game records. Training will not be conducted.',)
    parser.add_option('-r', '--record_file', dest='record_file', default=None, help='Game records already played')
    parser.add_option('-w', '--weight_file', dest='weight_file', default=None, help='Weights of neural network parameters')

    (options, args) = parser.parse_args()

    trainer = Trainer(options.port, options.store, options.record_file, options.weight_file)
    trainer.run()
