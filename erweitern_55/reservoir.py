import bisect
import minishogilib
import numpy as np
import os
import random
import simplejson
import threading

import gamerecord
import network


class Reservoir(object):
    def __init__(self, json_dump='records.json'):
        self.records = []
        self.learning_targets = []
        self.json_dump = json_dump
        self.lock = threading.Lock()

        if os.path.isfile(json_dump):
            self._load()

    def _load(self):
        with open(self.json_dump, 'r') as f:
            line = f.readline()

            while line:
                data = simplejson.loads(line)

                record = gamerecord.GameRecord()
                record.ply = data['ply']
                record.sfen_kif = data['sfen_kif']
                record.mcts_result = data['mcts_result']
                record.learning_target_plys = data['learning_target_plys']
                record.winner = data['winner']
                record.timestamp = data['timestamp']

                self.records.append(record)

                line = f.readline()

        self.learning_targets = []
        for record in self.records:
            self.learning_targets.append(record.learning_target_plys)

    def push(self, record):
        """Add a game record.
        """
        with self.lock:
            self.records.append(record)
            self.learning_targets.append(record.learning_target_plys)

            with open(self.json_dump, 'a') as f:
                simplejson.dump(record.to_dict(), f)
                f.write('\n')

    def sample(self, nn, mini_batch_size, recent, discard=True):
        """Sample positions from game records.

        # Arguments:
            mini_batch_size: the size of array of positions.
            recent: How many recent games are the target of sampling.

        # Returns:
            nninputs: the representation of the neural network input layer of positions.
            policies: the representation of distributions of MCTS outputs.
            values: the winners of games.
        """

        with self.lock:
            if discard:
                self.records = self.records[-recent:]
                self.learning_targets = self.learning_targets[-recent:]

            # Add index.
            recent_records = self.records[-recent:]
            recent_targets = self.learning_targets[-recent:]
            cumulative_plys = [0 for _ in range(recent + 1)]
            for i in range(recent):
                cumulative_plys[i + 1] = cumulative_plys[i] + \
                    len(recent_targets[i])
            indicies = random.sample(
                range(cumulative_plys[recent]), mini_batch_size)
            indicies.sort()
            target_plys = [None for _ in range(mini_batch_size)]
            lo = 0
            for i in range(mini_batch_size):
                index = bisect.bisect_right(
                    cumulative_plys, indicies[i], lo=lo) - 1
                ply = recent_targets[index][indicies[i] - cumulative_plys[index]]
                target_plys[i] = (recent_records[index], ply)
                lo = index

        nninputs = nn.zero_inputs(mini_batch_size)
        policies = np.zeros((mini_batch_size, 69 * 5 * 5), dtype='float32')
        values = np.zeros((mini_batch_size, 1), dtype='float32')

        for target_index in range(mini_batch_size):
            record = target_plys[target_index][0]
            sfen_kif = ' '.join(record.sfen_kif[:target_plys[target_index][1]])

            position = minishogilib.Position()
            position.set_sfen_without_startpos(sfen_kif)

            # Input.
            nninputs[target_index] = nn.get_inputs([position])[0]

            # Policy.
            sum_N, q, playouts = record.mcts_result[target_plys[target_index][1]]
            for playout in playouts:
                move = position.sfen_to_move(playout[0])
                policies[target_index][move.to_policy_index()] = playout[1] / sum_N

            # Value.
            if record.winner == 2:
                values[target_index] = 0
            elif record.winner == position.get_side_to_move():
                values[target_index] = 1
            else:
                values[target_index] = -1

        return nninputs, policies, values

    def len(self):
        with self.lock:
            return len(self.records)

    def len_learning_targets(self):
        with self.lock:
            flatten = [j for i in self.learning_targets for j in i]
        return len(flatten)
