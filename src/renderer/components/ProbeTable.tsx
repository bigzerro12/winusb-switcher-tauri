import React from "react";
import { Probe, DriverType } from "@shared/types";

interface ProbeTableProps {
  probes: Probe[];
  selectedProbeId: string | null;
  onSelectProbe: (id: string) => void;
}

export default function ProbeTable({ probes, selectedProbeId, onSelectProbe }: ProbeTableProps) {
  const getDriverBadge = (driver: DriverType) => {
    switch (driver) {
      case "SEGGER":  return <span className="driver-badge driver-segger">SEGGER</span>;
      case "WinUSB":  return <span className="driver-badge driver-winusb">WinUSB</span>;
      default:        return <span className="driver-badge driver-unknown">Unknown</span>;
    }
  };

  return (
    <table id="probe-table" className="probe-table">
      <thead>
        <tr>
          <th>SERIAL</th>
          <th>PRODUCT</th>
          <th>NICKNAME</th>
          <th>CONNECTION</th>
          <th>USB DRIVER</th>
          <th>PROBE FIRMWARE</th>
        </tr>
      </thead>
      <tbody>
        {probes.length > 0 ? (
          probes.map((probe) => (
            <tr
              key={probe.id}
              onClick={() => onSelectProbe(probe.id)}
              className={`probe-row ${selectedProbeId === probe.id ? "selected" : ""}`}
            >
              <td>{probe.serialNumber}</td>
              <td>{probe.productName}</td>
              <td>
                {probe.nickName && probe.nickName !== "<not set>"
                  ? probe.nickName
                  : <span style={{ color: "#adb5bd", fontStyle: "italic" }}>&lt;not set&gt;</span>
                }
              </td>
              <td>
                <div className="connection-status">
                  <span className="connection-dot"></span>
                  {probe.connection}
                </div>
              </td>
              <td>{getDriverBadge(probe.driver)}</td>
              <td>
                {probe.firmware
                  ? <span className="firmware-date">{probe.firmware}</span>
                  : <span className="firmware-unknown">—</span>
                }
              </td>
            </tr>
          ))
        ) : (
          <tr>
            <td colSpan={6} className="text-center text-gray-500 py-8">
              No J-Link probes detected
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}