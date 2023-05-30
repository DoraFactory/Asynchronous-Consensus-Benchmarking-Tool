import numpy as np
import pandas as pd
import os
import matplotlib.pyplot as plt
from mpl_toolkits.mplot3d import Axes3D

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

delays = [100, 200, 300]  # Define delay values

# Prepare figures
fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(10, 10), subplot_kw={'projection': '3d'})

for i, directory_path in enumerate(directory_paths):
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

    # Prepare data for 3D plot
    x = np.array(grouped.index)
    y = np.full_like(x, delays[i])  # Set delay value for each data point
    z = np.zeros_like(x)
    dx = 0.5
    dy = 30  # Width and depth of the bars
    dz1 = np.array(grouped['block num'])
    dz2 = np.array(grouped['epoch_time(latency)'])

    # Create 3D bar plots
    ax1.bar3d(x, y, z, dx, dy, dz1, shade=True)
    ax2.bar3d(x, y, z, dx, dy, dz2, shade=True)

# Set labels and titles
ax1.set_xlabel('Number of Nodes')
ax1.set_ylabel('Delay [ms]')
ax1.set_zlabel('Average Throughput')
ax1.set_title('Average Throughput vs Number of Nodes and Delay')

ax2.set_xlabel('Number of Nodes')
ax2.set_ylabel('Delay [ms]')
ax2.set_zlabel('Average Latency')
ax2.set_title('Average Latency vs Number of Nodes and Delay')

# Create the directory to save figures
save_dir = os.path.join(parent_dir, 'analysis-fig')
os.makedirs(save_dir, exist_ok=True)

# Save the figures
save_path = os.path.join(save_dir, 'threedimensionalpic.png')
fig.savefig(save_path)

# Show the plots
# plt.show()