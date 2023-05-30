import pandas as pd
import os
import matplotlib.pyplot as plt

# Define column names
column_names = ['epoch_id', 'validator num', 'contributor num', 'batch num', 'tx size(Bytes)/tx', 'block num', 'epoch_time(latency)']

current_dir = os.getcwd()
parent_dir = os.path.dirname(current_dir)

# List of directories
directory_paths = [
    parent_dir + '/test-data/test_data_7_30_1600_20_4_delay100/hbb-node1',
    parent_dir + '/test-data/test_data_7_30_1600_20_4_delay200/hbb-node1',
    parent_dir + '/test-data/test_data_7_30_1600_20_4_delay300/hbb-node1',
]

# Create a figure with two subplots
fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(10, 10))

for directory_path in directory_paths:
    data_frames = []
    for filename in os.listdir(directory_path):
        if filename.endswith(".md"):  # Only process .md files
            with open(os.path.join(directory_path, filename), 'r') as f:
                data = f.read()

            # Split the table into lines
            lines = data.split('\n')[3:-1]  # Exclude the header and the alignment line and the last empty line

            # Split each line into its components
            table_data = [line.split('|')[1:-1] for line in lines]  # Exclude the first and last empty strings

            # Convert the table data into a pandas DataFrame and append it to the list
            df = pd.DataFrame(table_data, columns=column_names)

            # Convert numeric columns to the correct data type
            for col in ['validator num', 'block num', 'epoch_time(latency)']:
                df[col] = pd.to_numeric(df[col])

            data_frames.append(df)

    # Concatenate all data frames
    df = pd.concat(data_frames)

    grouped = df.groupby('validator num').agg({'block num': 'mean', 'epoch_time(latency)': 'mean'})

    # Plot average throughput
    ax1.plot(grouped.index, grouped['block num'], label=directory_path)

    # Plot average latency
    ax2.plot(grouped.index, grouped['epoch_time(latency)'], label=directory_path)

# Finalize average throughput plot
ax1.set_xlabel('Number of Nodes')
ax1.set_ylabel('Average Throughput')
ax1.set_title('Average Throughput vs Number of Nodes')
ax1.legend()

# Finalize average latency plot
ax2.set_xlabel('Number of Nodes')
ax2.set_ylabel('Average Latency')
ax2.set_title('Average Latency vs Number of Nodes')
ax2.legend()

# Create the directory to save figures
save_dir = os.path.join(parent_dir, 'analysis-fig')
os.makedirs(save_dir, exist_ok=True)

# Save the figures
save_path = os.path.join(save_dir, 'threefileloading.png')
plt.savefig(save_path)

# plt.show()
