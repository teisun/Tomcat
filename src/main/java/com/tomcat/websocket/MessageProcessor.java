package com.tomcat.websocket;

import cn.hutool.json.JSONUtil;
import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.response.ChatResp;
import com.tomcat.controller.response.ChatAssistantResponse;
import com.tomcat.controller.response.Topic;
import com.tomcat.nettyws.pojo.Session;
import com.tomcat.service.AiCTutor;
import io.netty.handler.codec.http.websocketx.TextWebSocketFrame;
import lombok.extern.slf4j.Slf4j;

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
        ChatResp<List<Topic>> resp = aiClient.curriculumPlan(req);
        session.sendText(new TextWebSocketFrame(JSONUtil.toJsonStr(resp)));

    }

    private void startTopic(ChatReq req){
        ChatResp<ChatAssistantResponse> resp = aiClient.startTopic(req);
        session.sendText(new TextWebSocketFrame(JSONUtil.toJsonStr(resp)));
    }

    private void chat(ChatReq req){
        ChatResp<ChatAssistantResponse> resp = aiClient.chat(req);
        session.sendText(new TextWebSocketFrame(JSONUtil.toJsonStr(resp)));
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
            default:
                break;
        }



        //TODO
        //        检查缓存中是否存在上下文
        //        请求openai API 获得返回数据
        //        推送消息到订阅用户的设备上
        //        异步生成消息摘要用于搜索
    }



}
