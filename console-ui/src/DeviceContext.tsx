import {useEffect,useState} from 'react';
import {brokerBase,brokerHome,fleetDevice} from './brokerRoute';
import './deviceContext.css';

type Device={id:string;name:string;description:string;hostname?:string;platform?:string};
type Desktop={id:string;environment:string;user:string};

export function DeviceContext(){
 const desktopId=new URLSearchParams(location.search).get('desktop');
 const [device,setDevice]=useState<Device|null>(null),[desktop,setDesktop]=useState<Desktop|null>(null);
 const [unavailable,setUnavailable]=useState(false);
 useEffect(()=>{
  if(!fleetDevice)return;
  const controller=new AbortController();
  async function load(){
   try{
    const response=await fetch('/api/v1/devices',{signal:controller.signal});
    if(!response.ok)throw Error();
    const body=await response.json();const current=body.devices.find((d:Device)=>d.id===fleetDevice);
    if(!current)throw Error();setDevice(current);setUnavailable(false);
    if(desktopId){
     const sessions=await fetch(`${brokerBase}/desktops`,{signal:controller.signal});
     if(sessions.ok){const inventory=await sessions.json();setDesktop([...inventory.desktops,...(inventory.history??[])].find((d:Desktop)=>d.id===desktopId)??null)}
    }
   }catch{if(!controller.signal.aborted)setUnavailable(true)}
  }
  void load();return()=>controller.abort();
 },[desktopId]);
 if(!fleetDevice)return null;
 const name=device?.name??`Device ${fleetDevice.slice(0,8)}`;
 return <section className="device-context" aria-label="Connection details">
  <nav aria-label="Breadcrumb"><a href="/">Fleet console</a><span aria-hidden="true">/</span>{desktopId?<><a href={brokerHome}>{name}</a><span aria-hidden="true">/</span><span aria-current="page">Desktop {desktopId.slice(0,8)}</span></>:<span aria-current="page">{name}</span>}</nav>
  <div className="device-context-summary"><strong>{name}</strong>{device?.hostname&&<span>{device.hostname}</span>}{device?.platform&&<span>{device.platform}</span>}{desktop&&<span>{desktop.environment} · {desktop.user}</span>}</div>
  {device?.description&&<p>{device.description}</p>}
  {unavailable&&<p>Device details are currently unavailable.</p>}
 </section>;
}
