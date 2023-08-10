package com.tomcat.controller;

import com.tomcat.controller.request.AuthenticationRequest;
import com.tomcat.utils.JsonUtil;
import com.tomcat.utils.JwtUtil;
import org.junit.jupiter.api.Test;
import org.junit.runner.RunWith;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.http.MediaType;
import org.springframework.test.context.junit4.SpringRunner;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.MvcResult;
import org.springframework.test.web.servlet.ResultHandler;
import org.springframework.test.web.servlet.request.MockMvcRequestBuilders;
import org.springframework.test.web.servlet.result.MockMvcResultMatchers;

import static org.junit.jupiter.api.Assertions.*;

@SpringBootTest
@RunWith(SpringRunner.class)
@AutoConfigureMockMvc
class UserProfileControllerTest {

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private JwtUtil jwtUtil;

    @Autowired
    private JsonUtil jsonUtil;

    @Value("${jwt.tokenHeader}")
    private String tokenHeader;
    @Value("${jwt.tokenHead}")
    private String tokenHead;

    @Test
    void getByUserId() {
    }

    @Test
    void updateProfile() throws Exception{
        AuthenticationRequest request = new AuthenticationRequest("tom", "1234", "", "13823232232", "8888888");
        String token = jwtUtil.generateToken(1, request.username);
        String json = jsonUtil.toJson(request);

        mockMvc.perform(MockMvcRequestBuilders
                        .post("/profile/update")
                        .content(json.getBytes())
                        .accept(MediaType.APPLICATION_JSON)
                        .contentType(MediaType.APPLICATION_JSON_VALUE)
                        .header(tokenHeader,tokenHead+token)
                ).andExpect(MockMvcResultMatchers.status().isOk())
                .andDo(new ResultHandler() {
                    @Override
                    public void handle(MvcResult result) throws Exception {
                        System.out.println("Response body: " + result.getResponse().getContentAsString());
                    }
                });
    }
}