package com.tomcat.websocket;

import cn.hutool.json.JSONUtil;
import com.tomcat.config.LocalCache;
import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.response.*;
import com.tomcat.nettyws.pojo.Session;
import com.tomcat.service.AiCTutor;
import io.netty.handler.codec.http.websocketx.TextWebSocketFrame;
import lombok.extern.slf4j.Slf4j;

import java.util.ArrayList;
import java.util.List;

@Slf4j
public class MessageProcessor {

    AiCTutor aiClient;
    Session session;

    String uid;




    public MessageProcessor(AiCTutor aiClient, Session session, String uid){
        this.aiClient = aiClient;
        this.session = session;
        this.uid = uid;
    }

    private void chatInit(){
        ChatResp<String> resp = aiClient.chatInit(uid);
        session.sendText(new TextWebSocketFrame(JSONUtil.toJsonStr(resp)));
    }

    private void curriculumPlan(ChatReq req){
        ChatResp<Topics> resp = aiClient.curriculumPlan(req);
        sendText(req.getChatId(), resp);
    }

    private void startTopic(ChatReq req){
        ChatResp<ChatAssistantData> resp = aiClient.startTopic(req);
        sendText(req.getChatId(), resp);
    }

    private void chat(ChatReq req){
        ChatResp<ChatAssistantData> resp = aiClient.chat(req);
        sendText(req.getChatId(), resp);
    }

    private void generateTips(ChatReq req){
        ChatResp<TipsResp> resp = aiClient.generateTips(req);
        sendText(req.getChatId(), resp);
    }

    private void offlineMsg(ChatReq req){
        ChatResp<List<OfflineMsgDTO>> resp = aiClient.offlineMsg(req);
        sendText(req.getChatId(), resp);
    }

    private void sendText(String chatId, ChatResp resp){
        if(session.isOpen()){
            String jsonStr = JSONUtil.toJsonStr(resp);
            log.info("MessageProcessor sendText:\n" + jsonStr);
            session.sendText(JSONUtil.toJsonStr(resp));
        }else {
            log.info("MessageProcessor sendText: session not open, cache the message!");
            List<OfflineMsgDTO> msgs;
            if(LocalCache.MESSAGE_CACHE.containsKey(uid)){
                msgs = (List<OfflineMsgDTO>) LocalCache.MESSAGE_CACHE.get(uid);
            }else {
                msgs = new ArrayList<>();
                LocalCache.MESSAGE_CACHE.put(uid, msgs);
            }
            OfflineMsgDTO offlineMsg = new OfflineMsgDTO();
            offlineMsg.setChatId(chatId);
            offlineMsg.setMsg(JSONUtil.toJsonStr(resp.getData()));
            msgs.add(offlineMsg);
            LocalCache.MESSAGE_CACHE.put(uid, msgs);
        }

    }



    private void commandNotFound(String command){
        ChatResp<TipsResp> resp = new ChatResp<>();
        resp.setCode(404);
        resp.setDescribe("Command " + command + " not found!");
        String jsonStr = JSONUtil.toJsonStr(resp);
        session.sendText(new TextWebSocketFrame(jsonStr));
    }


    public void processor(String msg) {

        ChatReq chatReq = JSONUtil.toBean(msg, ChatReq.class);
        chatReq.setUid(uid);
        switch (chatReq.getCommand().toUpperCase()){
            case Command.CHAT_INIT:
                this.chatInit();
                break;
            case Command.CURRICULUM_PLAN:
                this.curriculumPlan(chatReq);
                break;
            case Command.START_TOPIC:
                this.startTopic(chatReq);
                break;
            case Command.CHAT:
                this.chat(chatReq);
                break;
            case Command.TIPS:
                this.generateTips(chatReq);
                break;
            case Command.OFFLINE_MSG:
                offlineMsg(chatReq);
                break;
            default:
                commandNotFound(chatReq.getCommand());
                break;
        }



        //TODO
        //        检查缓存中是否存在上下文
        //        请求openai API 获得返回数据
        //        推送消息到订阅用户的设备上
        //        异步生成消息摘要用于搜索
    }



}
